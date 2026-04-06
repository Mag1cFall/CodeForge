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
use crate::skill::manager::SkillManager;
use crate::tools::registry::ToolRegistry;

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
            db,
        })
    }
}
