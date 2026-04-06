use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::agent::definition::AgentStore;
use crate::config::app_config::AppConfig;
use crate::db::sqlite::Database;
use crate::error::AppResult;
use crate::harness::{budget::TokenBudget, permission::PermissionManager, sandbox::SandboxManager};
use crate::knowledge::{retriever::KnowledgeService, store::KnowledgeStore};
use crate::llm::store::ProviderStore;
use crate::logging::service::TraceLogService;
use crate::mcp::server_mgr::McpServerManager;
use crate::session::manager::SessionManager;
use crate::skill::manager::{SkillManager, SkillSyncSource};
use crate::tools::registry::ToolRegistry;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingPermissionContext {
    pub request_id: String,
    pub session_id: String,
    pub tool_name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: Database,
    pub providers: ProviderStore,
    pub agents: AgentStore,
    pub sessions: SessionManager,
    pub skills: SkillManager,
    pub mcp_servers: McpServerManager,
    pub knowledge: KnowledgeService,
    pub logs: TraceLogService,
    pub permission: PermissionManager,
    pub sandbox: SandboxManager,
    pub tools: ToolRegistry,
    pub budget: TokenBudget,
    pub pending_permissions: Arc<Mutex<HashMap<String, PendingPermissionContext>>>,
}

impl AppState {
    pub fn new(config: AppConfig) -> AppResult<Self> {
        let db = Database::new(&config.db_path)?;
        let sandbox = SandboxManager::new(config.sandbox_root.clone())?;
        let tools = ToolRegistry::new(sandbox.clone());
        Ok(Self {
            config: config.clone(),
            providers: ProviderStore::new(db.clone()),
            agents: AgentStore::new(db.clone()),
            sessions: SessionManager::new(db.clone()),
            skills: SkillManager::new(db.clone()),
            mcp_servers: McpServerManager::new(db.clone()),
            knowledge: KnowledgeService::new(KnowledgeStore::new(db.clone())),
            logs: TraceLogService::new(db.clone()),
            permission: PermissionManager::new(),
            sandbox,
            tools,
            budget: TokenBudget::new(128_000, 2_000_000),
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            db,
        })
    }

    pub fn initialize_defaults(&self) -> AppResult<()> {
        self.agents.ensure_default_agent()?;
        self.skills
            .ensure_default_skill_files(&self.config.builtin_skills_dir)?;
        let _ = self.skills.sync_from_dirs(&[
            SkillSyncSource {
                root: &self.config.builtin_skills_dir,
                default_enabled: true,
            },
            SkillSyncSource {
                root: &self.config.skills_dir,
                default_enabled: false,
            },
        ])?;
        let _ = self.providers.ensure_default_from_env()?;
        Ok(())
    }
}
