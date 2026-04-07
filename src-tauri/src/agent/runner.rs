use std::collections::BTreeMap;
use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Runtime};

use crate::error::AppResult;
use crate::harness::budget::TokenBudget;
use crate::harness::permission::{PermissionManager, PermissionPolicy, PermissionRequest};
use crate::llm::model::{configured_context_window, ChatMessage, ChatRequest, ToolDefinition};
use crate::llm::provider::build_provider;
use crate::llm::store::ProviderStore;
use crate::logging::service::TraceLogService;
use crate::session::manager::{SessionManager, SessionRecord};
use crate::tools::registry::{ToolExecutionContext, ToolRegistry};
use crate::tools::schema::ToolSet;

use super::definition::{AgentRecord, AgentStore, AgentStatus};
use super::hooks::{AgentHooks, TraceHooks};
use super::prompt::build_system_prompt;

const MAX_TOOL_ROUNDS: usize = 16;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunResult {
    pub content: String,
    pub tool_results: Vec<serde_json::Value>,
    pub permission_request: Option<PermissionRequest>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunConfig {
    pub provider_id: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub approved_tool_names: Vec<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct AgentRuntime {
    pub agent_store: AgentStore,
    pub provider_store: ProviderStore,
    pub tool_registry: ToolRegistry,
    pub session_manager: SessionManager,
    pub permission_manager: PermissionManager,
    pub budget: TokenBudget,
    pub logs: TraceLogService,
    pub context_window_overrides: BTreeMap<String, usize>,
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
        config: AgentRunConfig,
    ) -> AppResult<AgentRunResult> {
        self.run_internal(
            agent,
            session,
            Some(user_message),
            skill_instructions,
            workspace_root,
            true,
            config,
            |request| {
                app.emit("permission_request", &request).ok();
            },
            |message| {
                app.emit(
                    "chat_progress",
                    serde_json::json!({ "sessionId": session.id, "message": message }),
                )
                .ok();
            },
            |tool_payload| {
                app.emit(
                    "chat_tool_result",
                    serde_json::json!({ "sessionId": session.id, "tool": tool_payload }),
                )
                .ok();
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
        config: AgentRunConfig,
    ) -> AppResult<AgentRunResult> {
        self.run_internal(
            agent,
            session,
            Some(user_message),
            skill_instructions,
            workspace_root,
            true,
            config,
            |_| {},
            |_| {},
            |_| {},
        )
        .await
    }

    pub async fn run_from_session<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        agent: &AgentRecord,
        session: &SessionRecord,
        skill_instructions: String,
        workspace_root: Option<PathBuf>,
        config: AgentRunConfig,
    ) -> AppResult<AgentRunResult> {
        self.run_internal(
            agent,
            session,
            None,
            skill_instructions,
            workspace_root,
            false,
            config,
            |request| {
                app.emit("permission_request", &request).ok();
            },
            |message| {
                app.emit(
                    "chat_progress",
                    serde_json::json!({ "sessionId": session.id, "message": message }),
                )
                .ok();
            },
            |tool_payload| {
                app.emit(
                    "chat_tool_result",
                    serde_json::json!({ "sessionId": session.id, "tool": tool_payload }),
                )
                .ok();
            },
        )
        .await
    }

    pub async fn run_from_session_headless(
        &self,
        agent: &AgentRecord,
        session: &SessionRecord,
        skill_instructions: String,
        workspace_root: Option<PathBuf>,
        config: AgentRunConfig,
    ) -> AppResult<AgentRunResult> {
        self.run_internal(
            agent,
            session,
            None,
            skill_instructions,
            workspace_root,
            false,
            config,
            |_| {},
            |_| {},
            |_| {},
        )
        .await
    }

    async fn run_internal<F, G, H>(
        &self,
        agent: &AgentRecord,
        session: &SessionRecord,
        user_message: Option<String>,
        skill_instructions: String,
        workspace_root: Option<PathBuf>,
        append_user_message: bool,
        config: AgentRunConfig,
        mut on_permission: F,
        mut on_progress: G,
        mut on_tool_event: H,
    ) -> AppResult<AgentRunResult>
    where
        F: FnMut(PermissionRequest),
        G: FnMut(String),
        H: FnMut(serde_json::Value),
    {
        self.agent_store.set_status(&agent.id, AgentStatus::Running)?;
        let hooks = TraceHooks::new(self.logs.clone());
        hooks.on_agent_start(&session.id);
        on_progress("已开始处理请求。".into());

        if append_user_message {
            let text = user_message.as_deref().unwrap_or_default();
            self.session_manager
                .append_message(&session.id, "user", text, vec![])?;
        }
        let mut messages = self
            .session_manager
            .messages(&session.id)?
            .into_iter()
            .map(|message| ChatMessage {
                role: if message.role == "tool" {
                    "assistant".into()
                } else {
                    message.role
                },
                content: message.content,
            })
            .collect::<Vec<_>>();

        let context = super::context::AgentContextManager::new(super::context::ContextWindow::default());
        let all_tools = self.tool_registry.list();
        // agent.tools 作为白名单：非空时只加载声明的工具，空则全部加载
        let tool_schemas: Vec<_> = if agent.tools.is_empty() {
            all_tools
        } else {
            all_tools
                .into_iter()
                .filter(|t| agent.tools.iter().any(|name| name == &t.name))
                .collect()
        };
        let tool_set = ToolSet::new(tool_schemas.clone());
        let tool_defs = tool_schemas
            .iter()
            .map(|tool| ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect::<Vec<_>>();

        let provider_record = if let Some(provider_id) = config.provider_id.as_ref().filter(|value| !value.trim().is_empty()) {
            self.provider_store
                .get_by_id(provider_id)?
                .ok_or_else(|| crate::error::AppError::new("指定 Provider 不存在"))?
        } else {
            self.provider_store
                .get_default()?
                .ok_or_else(|| crate::error::AppError::new("没有可用的默认 Provider"))?
        };
        let provider = build_provider(provider_record.clone())?;
        let model = resolve_effective_model(&config, agent, &provider_record);
        let context_window = configured_context_window(
            &self.context_window_overrides,
            Some(&provider_record),
            &model,
        )
        .unwrap_or(
            self.provider_store
                .resolve_context_window_with_refresh(Some(&provider_record), &model)
                .await?,
        );
        self.session_manager
            .update_context_max(&session.id, context_window)?;

        let mut tool_results = Vec::new();
        let mut permission_request = None;
        let mut final_content = String::new();

        for round in 0..MAX_TOOL_ROUNDS {
            let snapshot = context.snapshot(&messages);
            self.budget.reserve(estimate_tokens(&messages))?;
            on_progress(format!("模型正在思考（第 {} 轮）...", round + 1));
            hooks.on_before_llm_call(messages.len());

            let response = match provider
                .chat(ChatRequest {
                    messages: snapshot.recent_messages.clone(),
                    system_prompt: Some(build_system_prompt(agent, &skill_instructions, &snapshot.summary, &tool_schemas)),
                    model: Some(model.clone()),
                    max_tokens: Some(config.max_tokens.unwrap_or(4096)),
                    temperature: Some(config.temperature.unwrap_or(0.2)),
                    top_p: config.top_p,
                    tools: tool_defs.clone(),
                })
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    let _ = self.logs.record(
                        "llm_chat_error",
                        serde_json::json!({
                            "sessionId": session.id,
                            "providerId": provider_record.id,
                            "providerType": provider_record.provider_type.as_str(),
                            "endpoint": provider_record.endpoint,
                            "model": model,
                            "messageCount": snapshot.recent_messages.len(),
                            "error": error.message,
                        }),
                    );
                    return Err(error);
                }
            };

            self.session_manager.update_usage(
                &session.id,
                response.usage.input_tokens,
            )?;
            hooks.on_after_llm_call(&response);

            if response.tool_calls.is_empty() {
                final_content = response.content.clone();
                self.session_manager
                    .append_message(&session.id, "assistant", &response.content, tool_results.clone())?;
                on_progress("已生成最终回复。".into());
                break;
            }

            for call in response.tool_calls {
                on_progress(format!("准备执行工具：{}", call.name));
                let schema = tool_set
                    .find(&call.name)
                    .cloned()
                    .ok_or_else(|| crate::error::AppError::new(format!("未知工具: {}", call.name)))?;
                hooks.on_before_tool_exec(&schema);

                let (policy, risk_level, description) = self.permission_manager.classify(&call.name);
                let approved_for_current_run = config
                    .approved_tool_names
                    .iter()
                    .any(|item| item.eq_ignore_ascii_case(&call.name));
                if policy == PermissionPolicy::AskUser && !approved_for_current_run {
                    let request = PermissionRequest {
                        id: call.id.clone(),
                        tool_name: call.name.clone(),
                        args: call.arguments.clone(),
                        risk_level,
                        description,
                    };
                    on_permission(request.clone());
                    permission_request = Some(request);
                    final_content = format!("等待权限确认：{}，确认后将继续执行。", call.name);
                    on_progress(final_content.clone());
                    break;
                }

                let result = match self.tool_registry.execute(
                    &call.name,
                    call.arguments.clone(),
                    &ToolExecutionContext {
                        workspace_root: workspace_root.clone(),
                    },
                ) {
                    Ok(result) => result,
                    Err(error) => {
                        let error_text = format!("工具 {} 执行失败：{}", schema.name, error.message);
                        let _ = self.logs.record(
                            "tool_exec_error",
                            serde_json::json!({
                                "sessionId": session.id,
                                "toolName": schema.name,
                                "error": error_text,
                            }),
                        );

                        let tool_payload = serde_json::json!({
                            "id": call.id,
                            "name": call.name,
                            "args": call.arguments,
                            "result": error_text,
                        });
                        tool_results.push(tool_payload.clone());
                        on_tool_event(tool_payload.clone());

                        self.session_manager.append_message(
                            &session.id,
                            "assistant",
                            &format!("Tool result:\n{}", tool_payload["result"].as_str().unwrap_or_default()),
                            vec![tool_payload.clone()],
                        )?;
                        context.append_tool_summary(
                            &mut messages,
                            &schema.name,
                            tool_payload["result"].as_str().unwrap_or_default(),
                        );
                        on_progress(format!("工具执行失败：{}，已继续后续流程。", schema.name));
                        continue;
                    }
                };

                hooks.on_after_tool_exec(&schema, &result);
                let tool_payload = serde_json::json!({
                    "id": call.id,
                    "name": call.name,
                    "args": call.arguments,
                    "result": result,
                });
                tool_results.push(tool_payload.clone());
                on_tool_event(tool_payload.clone());
                let summary = format!("Tool result:\n{}", tool_payload["result"].as_str().unwrap_or_default());
                self.session_manager
                    .append_message(&session.id, "assistant", &summary, vec![tool_payload.clone()])?;
                context.append_tool_summary(
                    &mut messages,
                    &schema.name,
                    tool_payload["result"].as_str().unwrap_or_default(),
                );
                on_progress(format!("已执行工具：{}", schema.name));
            }

            if permission_request.is_some() {
                break;
            }
        }

        if permission_request.is_none() && final_content.trim().is_empty() && !tool_results.is_empty() {
            on_progress("正在汇总工具执行结果...".into());
            let snapshot = context.snapshot(&messages);
            hooks.on_before_llm_call(messages.len());
            match provider
                .chat(ChatRequest {
                    messages: snapshot.recent_messages.clone(),
                    system_prompt: Some(build_system_prompt(agent, &skill_instructions, &snapshot.summary, &tool_schemas)),
                    model: Some(model.clone()),
                    max_tokens: Some(config.max_tokens.unwrap_or(4096)),
                    temperature: Some(config.temperature.unwrap_or(0.2)),
                    top_p: config.top_p,
                    tools: vec![],
                })
                .await
            {
                Ok(response) => {
                    hooks.on_after_llm_call(&response);
                    if !response.content.trim().is_empty() {
                        final_content = response.content.clone();
                        self.session_manager.append_message(
                            &session.id,
                            "assistant",
                            &final_content,
                            tool_results.clone(),
                        )?;
                        on_progress("已生成最终回复。".into());
                    }
                }
                Err(error) => {
                    let _ = self.logs.record(
                        "llm_chat_error",
                        serde_json::json!({
                            "sessionId": session.id,
                            "providerId": provider_record.id,
                            "providerType": provider_record.provider_type.as_str(),
                            "endpoint": provider_record.endpoint,
                            "model": model,
                            "messageCount": snapshot.recent_messages.len(),
                            "error": error.message,
                            "phase": "tool_result_summary",
                        }),
                    );
                }
            }
        }

        if permission_request.is_some() && !final_content.trim().is_empty() {
            self.session_manager
                .append_message(&session.id, "assistant", &final_content, vec![])?;
        }

        if permission_request.is_none() && final_content.trim().is_empty() {
            final_content = if tool_results.is_empty() {
                "本轮执行未生成可展示回复。".into()
            } else {
                "工具执行已完成，但模型未返回最终文本。".into()
            };
            self.session_manager
                .append_message(&session.id, "assistant", &final_content, tool_results.clone())?;
            on_progress("已结束执行。".into());
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

fn resolve_effective_model(
    config: &AgentRunConfig,
    agent: &AgentRecord,
    provider: &crate::llm::model::ProviderRecord,
) -> String {
    if let Some(model) = config.model.as_ref().filter(|m| !m.trim().is_empty()) {
        return model.clone();
    }

    let agent_model = agent.model.trim();
    if !agent_model.is_empty() {
        let models = &provider.extra.models;
        if models.is_empty() || models.iter().any(|m| m.eq_ignore_ascii_case(agent_model)) {
            return agent_model.to_string();
        }
    }

    provider.model.clone()
}

fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    messages.iter().map(|message| message.content.len() / 4 + 1).sum()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::db::sqlite::Database;
    use crate::harness::{budget::TokenBudget, permission::PermissionManager, sandbox::SandboxManager};
    use crate::llm::{model::{ProviderConfigInput, ProviderType}, store::ProviderStore};
    use crate::logging::service::TraceLogService;
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
            logs: TraceLogService::new(db.clone()),
            context_window_overrides: Default::default(),
        };

        let _ = runtime;
        let _ = session;
    }

    #[tokio::test]
    #[ignore]
    async fn live_runner_chat_simple() {
        let endpoint = std::env::var("CODEFORGE_LIVE_LLM_ENDPOINT").expect("CODEFORGE_LIVE_LLM_ENDPOINT required");
        let api_key = std::env::var("CODEFORGE_LIVE_LLM_API_KEY").expect("CODEFORGE_LIVE_LLM_API_KEY required");
        let model = std::env::var("CODEFORGE_LIVE_LLM_MODEL").expect("CODEFORGE_LIVE_LLM_MODEL required");

        let db_path = std::env::temp_dir().join(format!("codeforge-live-runner-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(&db_path).expect("db should initialize");
        let agent_store = AgentStore::new(db.clone());
        agent_store.ensure_default_agent().expect("default agent should exist");

        let provider_store = ProviderStore::new(db.clone());
        provider_store
            .create(ProviderConfigInput {
                name: "Live OpenAI Compatible".into(),
                provider_type: ProviderType::OpenAiCompatible,
                endpoint,
                api_key: Some(api_key),
                model: model.clone(),
                models: vec![model.clone()],
                enabled: true,
                is_default: true,
                headers: Default::default(),
            })
            .expect("provider should be stored");

        let session_manager = SessionManager::new(db.clone());
        let mut agent = agent_store.list().expect("agents should load")[0].clone();
        agent.model = model;
        let session = session_manager
            .create(agent.id.clone(), Some("live-runner".into()))
            .expect("session should be created");

        let sandbox = SandboxManager::new(std::env::temp_dir().join(format!("codeforge-live-runner-sandbox-{}", uuid::Uuid::new_v4()))).expect("sandbox should initialize");
        let runtime = AgentRuntime {
            agent_store,
            provider_store,
            tool_registry: ToolRegistry::new(sandbox),
            session_manager,
            permission_manager: PermissionManager::new(),
            budget: TokenBudget::new(100_000, 1_000_000),
            logs: TraceLogService::new(db.clone()),
            context_window_overrides: Default::default(),
        };

        let result = runtime
            .run_headless(
                &agent,
                &session,
                "你好".into(),
                String::new(),
                Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()),
                AgentRunConfig::default(),
            )
            .await
            .expect("live runner chat should succeed");

        assert!(!result.content.trim().is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn live_runner_permission_resume() {
        let endpoint = std::env::var("CODEFORGE_LIVE_LLM_ENDPOINT").expect("CODEFORGE_LIVE_LLM_ENDPOINT required");
        let api_key = std::env::var("CODEFORGE_LIVE_LLM_API_KEY").expect("CODEFORGE_LIVE_LLM_API_KEY required");
        let model = std::env::var("CODEFORGE_LIVE_LLM_MODEL").expect("CODEFORGE_LIVE_LLM_MODEL required");

        let db_path = std::env::temp_dir().join(format!("codeforge-live-runner-perm-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(&db_path).expect("db should initialize");
        let agent_store = AgentStore::new(db.clone());
        agent_store.ensure_default_agent().expect("default agent should exist");

        let provider_store = ProviderStore::new(db.clone());
        provider_store
            .create(ProviderConfigInput {
                name: "Live OpenAI Compatible".into(),
                provider_type: ProviderType::OpenAiCompatible,
                endpoint,
                api_key: Some(api_key),
                model: model.clone(),
                models: vec![model.clone()],
                enabled: true,
                is_default: true,
                headers: Default::default(),
            })
            .expect("provider should be stored");

        let session_manager = SessionManager::new(db.clone());
        let mut agent = agent_store.list().expect("agents should load")[0].clone();
        agent.model = model;
        let session = session_manager
            .create(agent.id.clone(), Some("live-runner-permission".into()))
            .expect("session should be created");

        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
        let sandbox = SandboxManager::new(std::env::temp_dir().join(format!("codeforge-live-runner-perm-sandbox-{}", uuid::Uuid::new_v4()))).expect("sandbox should initialize");
        let tools = ToolRegistry::new(sandbox);
        let runtime = AgentRuntime {
            agent_store,
            provider_store,
            tool_registry: tools.clone(),
            session_manager: session_manager.clone(),
            permission_manager: PermissionManager::new(),
            budget: TokenBudget::new(100_000, 1_000_000),
            logs: TraceLogService::new(db.clone()),
            context_window_overrides: Default::default(),
        };

        let first = runtime
            .run_headless(
                &agent,
                &session,
                "你在哪个目录现在？列出一下文件".into(),
                String::new(),
                Some(repo_root.clone()),
                AgentRunConfig::default(),
            )
            .await
            .expect("first permission phase should succeed");

        let request = first.permission_request.expect("permission request should exist");
        let output = tools
            .execute(
                &request.tool_name,
                request.args,
                &ToolExecutionContext {
                    workspace_root: Some(repo_root.clone()),
                },
            )
            .expect("approved tool should execute");
        session_manager
            .append_message(
                &session.id,
                "assistant",
                &format!("Tool result:\n{}", output),
                vec![serde_json::json!({
                    "id": request.id,
                    "name": request.tool_name,
                    "result": output,
                })],
            )
            .expect("tool result should persist");

        let second = runtime
            .run_from_session_headless(&agent, &session, String::new(), Some(repo_root), AgentRunConfig::default())
            .await
            .expect("resume phase should succeed");

        assert!(!second.content.trim().is_empty());
    }
}
