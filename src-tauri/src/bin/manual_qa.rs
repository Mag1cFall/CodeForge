use codeforge_lib::agent::{definition::AgentStore, runner::AgentRuntime};
use codeforge_lib::commands::project::{clone_repo, collect_review_issues};
use codeforge_lib::db::sqlite::Database;
use codeforge_lib::harness::{budget::TokenBudget, permission::PermissionManager, sandbox::SandboxManager};
use codeforge_lib::knowledge::{retriever::KnowledgeService, store::KnowledgeStore};
use codeforge_lib::llm::{model::{ProviderConfigInput, ProviderType}, store::ProviderStore};
use codeforge_lib::logging::service::{TraceLogFilter, TraceLogService};
use codeforge_lib::session::manager::SessionManager;
use codeforge_lib::skill::manager::SkillManager;
use codeforge_lib::tools::registry::{ToolExecutionContext, ToolRegistry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("CODEFORGE_LIVE_LLM_ENDPOINT")?;
    let api_key = std::env::var("CODEFORGE_LIVE_LLM_API_KEY")?;
    let model = std::env::var("CODEFORGE_LIVE_LLM_MODEL")?;

    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root should exist")
        .to_path_buf();
    let work_root = std::env::temp_dir().join(format!("codeforge-manual-qa-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&work_root)?;

    let db = Database::new(work_root.join("qa.db"))?;
    let logs = TraceLogService::new(db.clone());
    let providers = ProviderStore::new(db.clone());
    let agents = AgentStore::new(db.clone());
    let sessions = SessionManager::new(db.clone());
    let sandbox = SandboxManager::new(work_root.join("sandbox"))?;
    let tools = ToolRegistry::new(sandbox.clone());
    let knowledge = KnowledgeService::new(KnowledgeStore::new(db.clone()));
    let skills = SkillManager::new(db.clone());
    let _ = skills.sync_from_dir(&repo_root.join("skills"));

    agents.ensure_default_agent()?;
    let provider = providers.create(ProviderConfigInput {
        name: "Live OpenAI Compatible".into(),
        provider_type: ProviderType::OpenAiCompatible,
        endpoint,
        api_key: Some(api_key),
        model: model.clone(),
        models: vec![model.clone()],
        enabled: true,
        is_default: true,
        headers: Default::default(),
    })?;
    logs.record("provider_create", serde_json::json!({ "id": provider.id, "name": provider.name }))?;

    let agent = agents.list()?.into_iter().next().expect("default agent should exist");
    let session = sessions.create(agent.id.clone(), Some("manual qa".into()))?;
    let runtime = AgentRuntime {
        agent_store: agents.clone(),
        provider_store: providers.clone(),
        tool_registry: tools.clone(),
        session_manager: sessions.clone(),
        permission_manager: PermissionManager::new(),
        budget: TokenBudget::new(128_000, 2_000_000),
    };

    let lib_rs = repo_root.join("src-tauri").join("src").join("lib.rs");
    let tool_output = tools.execute(
        "read_file",
        serde_json::json!({ "path": lib_rs.display().to_string() }),
        &ToolExecutionContext { workspace_root: None },
    )?;
    logs.record("tool", serde_json::json!({ "name": "read_file", "preview": tool_output.lines().take(3).collect::<Vec<_>>() }))?;

    let chat = runtime
        .run_headless(
            &agent,
            &session,
            format!("请先调用 read_file 读取路径 {}，再只回复文件里注册的第一个 command 名称。", lib_rs.display()),
            skills.active_instructions().unwrap_or_default(),
            Some(repo_root.clone()),
        )
        .await?;
    logs.record("chat", serde_json::json!({ "content": chat.content, "toolResults": chat.tool_results }))?;

    let indexed = knowledge.index_repo(&repo_root)?;
    let search = knowledge.search("agent loop", 3)?;
    logs.record("knowledge_search", serde_json::json!({ "repo": indexed.path, "results": search.len() }))?;

    let local_review = collect_review_issues(&repo_root)?;
    logs.record("project_review", serde_json::json!({ "path": repo_root.display().to_string(), "issueCount": local_review.len() }))?;

    let remote_root = work_root.join("remote-codeforge");
    clone_repo("https://github.com/Mag1cFall/CodeForge", &remote_root)?;
    let remote_review = collect_review_issues(&remote_root)?;
    logs.record("project_clone", serde_json::json!({ "path": remote_root.display().to_string(), "issueCount": remote_review.len() }))?;

    println!("QA_PROVIDER={}", serde_json::to_string(&provider)?);
    println!("QA_TOOL_READ={}", tool_output.lines().take(3).collect::<Vec<_>>().join(" | "));
    println!("QA_CHAT={}", chat.content);
    println!("QA_CHAT_TOOL_RESULTS={}", serde_json::to_string(&chat.tool_results)?);
    println!("QA_KNOWLEDGE_INDEX={}", serde_json::to_string(&indexed)?);
    println!("QA_KNOWLEDGE_SEARCH={}", serde_json::to_string(&search)?);
    println!("QA_LOCAL_REVIEW_COUNT={}", local_review.len());
    println!("QA_LOCAL_REVIEW_FIRST={}", serde_json::to_string(&local_review.first())?);
    println!("QA_REMOTE_REVIEW_COUNT={}", remote_review.len());
    println!("QA_REMOTE_REVIEW_FIRST={}", serde_json::to_string(&remote_review.first())?);
    println!("QA_LOGS={}", serde_json::to_string(&logs.list(TraceLogFilter { kind: None, limit: Some(10) })?)?);

    Ok(())
}
