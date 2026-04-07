use std::path::PathBuf;

use codeforge_lib::agent::{
    definition::AgentStore,
    runner::{AgentRunConfig, AgentRuntime},
};
use codeforge_lib::commands::project::collect_review_issues;
use codeforge_lib::commands::settings::AppSettings;
use codeforge_lib::db::sqlite::Database;
use codeforge_lib::harness::{
    budget::TokenBudget, permission::PermissionManager, sandbox::SandboxManager,
};
use codeforge_lib::knowledge::{retriever::KnowledgeService, store::KnowledgeStore};
use codeforge_lib::llm::{
    model::{ProviderConfigInput, ProviderType},
    store::ProviderStore,
};
use codeforge_lib::logging::service::{TraceLogFilter, TraceLogService};
use codeforge_lib::mcp::server_mgr::{McpServerConfigInput, McpServerManager};
use codeforge_lib::session::manager::SessionManager;
use codeforge_lib::skill::manager::SkillManager;
use codeforge_lib::tools::registry::{ToolExecutionContext, ToolRegistry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("CODEFORGE_LIVE_LLM_ENDPOINT")?;
    let api_key = std::env::var("CODEFORGE_LIVE_LLM_API_KEY")?;
    let model = std::env::var("CODEFORGE_LIVE_LLM_MODEL")?;

    let work_root =
        std::env::temp_dir().join(format!("codeforge-manual-qa-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&work_root)?;
    let fixture_root = create_fixture_project(&work_root)?;

    println!("[manual-qa] fixture={}", fixture_root.display());

    let db = Database::new(work_root.join("qa.db"))?;
    let logs = TraceLogService::new(db.clone());
    let providers = ProviderStore::new(db.clone());
    let agents = AgentStore::new(db.clone());
    let sessions = SessionManager::new(db.clone());
    let skills = SkillManager::new(db.clone());
    let mcp_servers = McpServerManager::new(db.clone());
    let sandbox = SandboxManager::new(work_root.join("sandbox"))?;
    let tools = ToolRegistry::new(sandbox.clone());
    let knowledge = KnowledgeService::new(KnowledgeStore::new(db.clone()));

    let settings = AppSettings {
        theme: "dark".into(),
        language: "zh-CN".into(),
        project_path: Some(fixture_root.display().to_string()),
        skills_path: Some(work_root.join("skills").display().to_string()),
        context_window_overrides: Default::default(),
    };
    db.set_json(
        "app_settings",
        &serde_json::to_string(&settings)?,
        &chrono::Utc::now().to_rfc3339(),
    )?;
    println!("[manual-qa] settings persisted");

    let builtin_skill_root = work_root.join("builtin-skills");
    let custom_skill_root = work_root.join("skills");
    skills.ensure_default_skill_files(&builtin_skill_root)?;
    std::fs::create_dir_all(custom_skill_root.join("custom-demo"))?;
    std::fs::write(
        custom_skill_root.join("custom-demo").join("SKILL.md"),
        "---\nname: custom-demo\ndescription: custom smoke skill\n---\nUse this only for smoke tests.\n",
    )?;
    let _ = skills.sync_from_dir(&builtin_skill_root, true)?;
    let _ = skills.sync_from_dir(&custom_skill_root, false)?;
    skills.toggle("custom-demo", true)?;
    println!("[manual-qa] skills synced and toggled");

    println!("[manual-qa] initializing defaults");
    agents.ensure_default_agent()?;
    let created_agent = agents.create(codeforge_lib::agent::definition::AgentConfigInput {
        name: "Smoke Agent".into(),
        instructions: Some("用于功能测试".into()),
        tools: vec!["read_file".into(), "run_shell".into()],
        model: model.clone(),
    })?;
    agents.set_status(
        &created_agent.id,
        codeforge_lib::agent::definition::AgentStatus::Running,
    )?;
    agents.set_status(
        &created_agent.id,
        codeforge_lib::agent::definition::AgentStatus::Stopped,
    )?;
    println!("[manual-qa] agent create/start/stop ok");

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
    logs.record(
        "provider_create",
        serde_json::json!({ "id": provider.id, "name": provider.name }),
    )?;

    let mcp = mcp_servers.add(McpServerConfigInput {
        name: "fixture-mcp".into(),
        transport: "stdio".into(),
        command: Some("cmd".into()),
        url: None,
        args: vec!["/C".into(), "exit 0".into()],
        env: Default::default(),
        headers: Default::default(),
        enabled: true,
    })?;
    let mcp_list_before_remove = mcp_servers.list()?;
    mcp_servers.remove(&mcp.id)?;
    let mcp_list_after_remove = mcp_servers.list()?;
    println!("[manual-qa] mcp add/list/remove ok");

    let mut agent = agents
        .list()?
        .into_iter()
        .next()
        .expect("default agent should exist");
    agent.model = model.clone();
    let session = sessions.create(agent.id.clone(), Some("manual qa".into()))?;
    let read_file_session =
        sessions.create(agent.id.clone(), Some("manual qa read-file".into()))?;
    let runtime = AgentRuntime {
        agent_store: agents.clone(),
        provider_store: providers.clone(),
        tool_registry: tools.clone(),
        session_manager: sessions.clone(),
        permission_manager: PermissionManager::new(),
        budget: TokenBudget::new(100_000, 1_000_000),
        logs: logs.clone(),
        context_window_overrides: Default::default(),
    };

    println!("[manual-qa] read package.json");
    let package_json = fixture_root.join("package.json");
    let tool_output = tools.execute(
        "read_file",
        serde_json::json!({ "path": package_json.display().to_string() }),
        &ToolExecutionContext {
            workspace_root: Some(fixture_root.clone()),
        },
    )?;
    logs.record(
        "tool",
        serde_json::json!({ "name": "read_file", "preview": tool_output.lines().take(3).collect::<Vec<_>>() }),
    )?;

    println!("[manual-qa] permission flow start");
    let permission_run = runtime
        .run_headless(
            &agent,
            &session,
            "你在哪个目录现在？列出一下文件".into(),
            String::new(),
            Some(fixture_root.clone()),
            AgentRunConfig::default(),
        )
        .await?;
    let permission = permission_run
        .permission_request
        .clone()
        .expect("permission flow should request approval");
    logs.record(
        "chat_permission",
        serde_json::json!({ "request": permission }),
    )?;

    let approved_output = tools.execute(
        &permission.tool_name,
        permission.args.clone(),
        &ToolExecutionContext {
            workspace_root: Some(fixture_root.clone()),
        },
    )?;
    let tool_payload = serde_json::json!({
        "id": permission.id,
        "name": permission.tool_name,
        "result": approved_output,
    });
    sessions.append_message(
        &session.id,
        "assistant",
        &format!(
            "Tool result:\n{}",
            tool_payload["result"].as_str().unwrap_or_default()
        ),
        vec![tool_payload.clone()],
    )?;

    let chat = runtime
        .run_from_session_headless(
            &agent,
            &session,
            String::new(),
            Some(fixture_root.clone()),
            AgentRunConfig::default(),
        )
        .await?;
    logs.record(
        "chat",
        serde_json::json!({ "content": chat.content, "toolResults": chat.tool_results, "approvedTool": tool_payload }),
    )?;

    println!("[manual-qa] read_file tool chat flow");
    let read_file_chat = runtime
        .run_headless(
            &agent,
            &read_file_session,
            "请读取当前项目的 package.json 文件内容，并告诉我 name 字段。".into(),
            String::new(),
            Some(fixture_root.clone()),
            AgentRunConfig::default(),
        )
        .await?;
    logs.record(
        "chat_read_file",
        serde_json::json!({ "content": read_file_chat.content, "toolResults": read_file_chat.tool_results, "permissionRequest": read_file_chat.permission_request }),
    )?;

    println!("[manual-qa] knowledge index/search");
    let indexed = knowledge.index_repo(&fixture_root)?;
    let search = knowledge.search("Agent Loop", 3)?;
    logs.record(
        "knowledge_search",
        serde_json::json!({ "repo": indexed.path, "results": search.len() }),
    )?;

    println!("[manual-qa] project review");
    let review_issues = collect_review_issues(&fixture_root)?;
    logs.record(
        "project_review",
        serde_json::json!({ "path": fixture_root.display().to_string(), "issueCount": review_issues.len() }),
    )?;

    println!("QA_PROVIDER={}", serde_json::to_string(&provider)?);
    println!("QA_SETTINGS={}", serde_json::to_string(&settings)?);
    println!("QA_SKILLS={}", serde_json::to_string(&skills.list()?)?);
    println!(
        "QA_MCP_BEFORE_REMOVE={}",
        serde_json::to_string(&mcp_list_before_remove)?
    );
    println!(
        "QA_MCP_AFTER_REMOVE={}",
        serde_json::to_string(&mcp_list_after_remove)?
    );
    println!(
        "QA_AGENT_CREATED={}",
        serde_json::to_string(&created_agent)?
    );
    println!(
        "QA_TOOL_READ={}",
        tool_output.lines().take(3).collect::<Vec<_>>().join(" | ")
    );
    println!("QA_CHAT={}", chat.content);
    println!(
        "QA_CHAT_TOOL_RESULTS={}",
        serde_json::to_string(&chat.tool_results)?
    );
    println!("QA_CHAT_READ_FILE={}", read_file_chat.content);
    println!(
        "QA_CHAT_READ_FILE_TOOL_RESULTS={}",
        serde_json::to_string(&read_file_chat.tool_results)?
    );
    println!("QA_KNOWLEDGE_INDEX={}", serde_json::to_string(&indexed)?);
    println!("QA_KNOWLEDGE_SEARCH={}", serde_json::to_string(&search)?);
    println!("QA_REVIEW_COUNT={}", review_issues.len());
    println!(
        "QA_REVIEW_FIRST={}",
        serde_json::to_string(&review_issues.first())?
    );
    println!(
        "QA_LOGS={}",
        serde_json::to_string(&logs.list(TraceLogFilter {
            kind: None,
            limit: Some(20),
        })?)?
    );

    Ok(())
}

fn create_fixture_project(work_root: &PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let fixture = work_root.join("fixture-project");
    std::fs::create_dir_all(fixture.join("src"))?;
    std::fs::write(
        fixture.join("package.json"),
        r#"{
  "name": "fixture-project",
  "version": "1.0.0",
  "scripts": {
    "lint": "echo lint"
  }
}"#,
    )?;
    std::fs::write(
        fixture.join("src").join("main.rs"),
        "fn main() { let value = Some(1).unwrap(); println!(\"{}\", value); panic!(\"demo panic\"); }",
    )?;
    std::fs::write(
        fixture.join("README.md"),
        "# Agent Loop\n\nThis fixture repository is used for CodeForge functional smoke tests.\n",
    )?;
    Ok(fixture)
}
