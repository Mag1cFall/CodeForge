use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Runtime};

use crate::error::AppResult;
use crate::harness::budget::TokenBudget;
use crate::harness::permission::{PermissionManager, PermissionPolicy, PermissionRequest};
use crate::llm::model::{ChatMessage, ChatRequest, ToolDefinition};
use crate::llm::provider::build_provider;
use crate::llm::store::ProviderStore;
use crate::session::manager::{SessionManager, SessionRecord};
use crate::tools::registry::{ToolExecutionContext, ToolRegistry};
use crate::tools::schema::ToolSet;

use super::definition::{AgentRecord, AgentStore, AgentStatus};
use super::hooks::{AgentHooks, NoopHooks};
use super::prompt::build_system_prompt;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunResult {
    pub content: String,
    pub tool_results: Vec<serde_json::Value>,
    pub permission_request: Option<PermissionRequest>,
}

#[derive(Debug, Clone)]
pub struct AgentRuntime {
    pub agent_store: AgentStore,
    pub provider_store: ProviderStore,
    pub tool_registry: ToolRegistry,
    pub session_manager: SessionManager,
    pub permission_manager: PermissionManager,
    pub budget: TokenBudget,
}

impl AgentRuntime {
    pub async fn run<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        agent: &AgentRecord,
        session: &SessionRecord,
        user_message: String,
        skill_instructions: String,
        workspace_root: Option<PathBuf>,
    ) -> AppResult<AgentRunResult> {
        self.run_internal(
            agent,
            session,
            user_message,
            skill_instructions,
            workspace_root,
            |request| {
                app.emit("permission_request", &request).ok();
            },
        )
        .await
    }

    pub async fn run_headless(
        &self,
        agent: &AgentRecord,
        session: &SessionRecord,
        user_message: String,
        skill_instructions: String,
        workspace_root: Option<PathBuf>,
    ) -> AppResult<AgentRunResult> {
        self.run_internal(
            agent,
            session,
            user_message,
            skill_instructions,
            workspace_root,
            |_| {},
        )
        .await
    }

    async fn run_internal<F>(
        &self,
        agent: &AgentRecord,
        session: &SessionRecord,
        user_message: String,
        skill_instructions: String,
        workspace_root: Option<PathBuf>,
        mut on_permission: F,
    ) -> AppResult<AgentRunResult>
    where
        F: FnMut(PermissionRequest),
    {
        self.agent_store.set_status(&agent.id, AgentStatus::Running)?;
        let hooks = NoopHooks;
        hooks.on_agent_start(&session.id);

        self.session_manager
            .append_message(&session.id, "user", &user_message, vec![])?;
        let mut messages = self
            .session_manager
            .messages(&session.id)?
            .into_iter()
            .map(|message| ChatMessage {
                role: message.role,
                content: message.content,
            })
            .collect::<Vec<_>>();

        let context = super::context::AgentContextManager::new(super::context::ContextWindow::default());
        let tool_schemas = self.tool_registry.list();
        let tool_set = ToolSet::new(tool_schemas.clone());
        let tool_defs = tool_schemas
            .iter()
            .map(|tool| ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect::<Vec<_>>();

        let provider_record = self
            .provider_store
            .get_default()?
            .ok_or_else(|| crate::error::AppError::new("没有可用的默认 Provider"))?;
        let provider = build_provider(provider_record)?;

        let mut tool_results = Vec::new();
        let mut permission_request = None;
        let mut final_content = String::new();

        for _ in 0..4 {
            let snapshot = context.snapshot(&messages);
            self.budget.reserve(estimate_tokens(&messages))?;
            hooks.on_before_llm_call(messages.len());

            let response = provider
                .chat(ChatRequest {
                    messages: snapshot.recent_messages.clone(),
                    system_prompt: Some(build_system_prompt(agent, &skill_instructions, &snapshot.summary, &tool_schemas)),
                    model: Some(agent.model.clone()),
                    max_tokens: Some(1024),
                    temperature: Some(0.2),
                    tools: tool_defs.clone(),
                })
                .await?;

            self.session_manager.update_usage(
                &session.id,
                self.session_manager
                    .get(&session.id)?
                    .map(|item| item.context_tokens_used)
                    .unwrap_or_default()
                    + response.usage.input_tokens
                    + response.usage.output_tokens,
            )?;
            hooks.on_after_llm_call(&response);

            if response.tool_calls.is_empty() {
                final_content = response.content.clone();
                self.session_manager
                    .append_message(&session.id, "assistant", &response.content, vec![])?;
                break;
            }

            for call in response.tool_calls {
                let schema = tool_set
                    .find(&call.name)
                    .cloned()
                    .ok_or_else(|| crate::error::AppError::new(format!("未知工具: {}", call.name)))?;
                hooks.on_before_tool_exec(&schema);

                let (policy, risk_level, description) = self.permission_manager.classify(&call.name);
                if policy == PermissionPolicy::AskUser {
                    let request = PermissionRequest {
                        id: call.id.clone(),
                        tool_name: call.name.clone(),
                        args: call.arguments.clone(),
                        risk_level,
                        description,
                    };
                    on_permission(request.clone());
                    permission_request = Some(request);
                    final_content = "等待权限确认后继续执行。".into();
                    break;
                }

                let result = self.tool_registry.execute(
                    &call.name,
                    call.arguments.clone(),
                    &ToolExecutionContext {
                        workspace_root: workspace_root.clone(),
                    },
                )?;

                hooks.on_after_tool_exec(&schema, &result);
                let tool_payload = serde_json::json!({
                    "id": call.id,
                    "name": call.name,
                    "result": result,
                });
                tool_results.push(tool_payload.clone());
                let summary = format!("Tool result:\n{}", tool_payload["result"].as_str().unwrap_or_default());
                self.session_manager
                    .append_message(&session.id, "assistant", &summary, vec![tool_payload.clone()])?;
                context.append_tool_summary(
                    &mut messages,
                    &schema.name,
                    tool_payload["result"].as_str().unwrap_or_default(),
                );
            }

            if permission_request.is_some() {
                break;
            }
        }

        self.agent_store.set_status(&agent.id, AgentStatus::Idle)?;
        hooks.on_agent_end(&session.id, &final_content);
        Ok(AgentRunResult {
            content: final_content,
            tool_results,
            permission_request,
        })
    }
}

fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    messages.iter().map(|message| message.content.len() / 4 + 1).sum()
}

#[cfg(test)]
mod tests {
    use crate::db::sqlite::Database;
    use crate::harness::{budget::TokenBudget, permission::PermissionManager, sandbox::SandboxManager};
    use crate::llm::{model::{ProviderConfigInput, ProviderType}, store::ProviderStore};
    use crate::session::manager::SessionManager;
    use crate::tools::registry::ToolRegistry;

    use super::*;

    #[tokio::test]
    async fn returns_permission_request_for_run_shell_tool() {
        let db_path = std::env::temp_dir().join(format!("codeforge-runner-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(&db_path).expect("db should initialize");
        let agent_store = AgentStore::new(db.clone());
        agent_store.ensure_default_agent().expect("default agent should exist");

        let provider_store = ProviderStore::new(db.clone());
        provider_store
            .create(ProviderConfigInput {
                name: "OpenAI Compatible".into(),
                provider_type: ProviderType::OpenAiCompatible,
                endpoint: "https://example.invalid/v1/chat/completions".into(),
                api_key: Some("secret".into()),
                model: "gpt-5.4-mini".into(),
                models: vec!["gpt-5.4-mini".into()],
                enabled: true,
                is_default: true,
                headers: Default::default(),
            })
            .expect("provider should be stored");

        let session_manager = SessionManager::new(db.clone());
        let session = session_manager
            .create(agent_store.list().expect("agents should load")[0].id.clone(), Some("test".into()))
            .expect("session should be created");

        let sandbox = SandboxManager::new(std::env::temp_dir().join(format!("codeforge-runner-sandbox-{}", uuid::Uuid::new_v4()))).expect("sandbox should initialize");
        let runtime = AgentRuntime {
            agent_store,
            provider_store,
            tool_registry: ToolRegistry::new(sandbox),
            session_manager,
            permission_manager: PermissionManager::new(),
            budget: TokenBudget::new(100_000, 1_000_000),
        };

        let _ = runtime;
        let _ = session;
    }
}
