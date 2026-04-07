use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::sqlite::Database;
use crate::error::{AppError, AppResult};

use super::hooks::AgentHooksConfig;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentStatus {
    Idle,
    Running,
    Stopped,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRecord {
    pub id: String,
    pub name: String,
    pub instructions: Option<String>,
    pub tools: Vec<String>,
    pub model: String,
    pub hooks: AgentHooksConfig,
    pub status: AgentStatus,
    pub is_system: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfigInput {
    pub name: String,
    pub instructions: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct AgentStore {
    db: Database,
}

impl AgentStore {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn ensure_default_agent(&self) -> AppResult<()> {
        let existing = self.list()?;
        if !existing.is_empty() {
            return Ok(());
        }

        for (config, system) in default_agents() {
            self.create_internal(config, system)?;
        }
        Ok(())
    }

    pub fn list(&self) -> AppResult<Vec<AgentRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, name, instructions, tools_json, model, status, hooks_json,
                   is_system, created_at, updated_at
            FROM agents ORDER BY is_system DESC, updated_at DESC
            "#,
        )?;

        let rows = statement.query_map([], |row| {
            let tools_json: String = row.get(3)?;
            let hooks_json: String = row.get(6)?;
            let status: String = row.get(5)?;
            Ok(AgentRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                instructions: row.get(2)?,
                tools: serde_json::from_str(&tools_json).unwrap_or_default(),
                model: row.get(4)?,
                hooks: serde_json::from_str(&hooks_json).unwrap_or_default(),
                status: match status.as_str() {
                    "running" => AgentStatus::Running,
                    "stopped" => AgentStatus::Stopped,
                    _ => AgentStatus::Idle,
                },
                is_system: row.get::<_, i64>(7).unwrap_or(0) != 0,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;

        let mut agents = Vec::new();
        for row in rows {
            agents.push(row?);
        }
        Ok(agents)
    }

    pub fn create(&self, input: AgentConfigInput) -> AppResult<AgentRecord> {
        self.create_internal(input, false)
    }

    fn create_internal(&self, input: AgentConfigInput, is_system: bool) -> AppResult<AgentRecord> {
        let name = input.name.trim();
        let model = input.model.trim();
        if name.is_empty() || model.is_empty() {
            return Err(AppError::new("Agent 名称与模型不能为空"));
        }

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let connection = self.db.connection()?;
        connection.execute(
            r#"
            INSERT INTO agents (id, name, instructions, tools_json, model, status, hooks_json, is_system, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, 'idle', ?6, ?7, ?8, ?9)
            "#,
            params![
                id,
                name,
                input.instructions,
                serde_json::to_string(&input.tools)?,
                model,
                serde_json::to_string(&AgentHooksConfig::default())?,
                if is_system { 1i64 } else { 0i64 },
                now,
                now,
            ],
        )?;
        self.get(&id)?
            .ok_or_else(|| AppError::new("新建 Agent 后未能读取记录"))
    }

    pub fn update(&self, id: &str, input: AgentConfigInput) -> AppResult<AgentRecord> {
        let existing = self
            .get(id)?
            .ok_or_else(|| AppError::new("指定 Agent 不存在"))?;

        let name = input.name.trim();
        let model = input.model.trim();
        if name.is_empty() || model.is_empty() {
            return Err(AppError::new("Agent 名称与模型不能为空"));
        }

        if existing.is_system && name != existing.name {
            return Err(AppError::new("系统 Agent 不允许修改名称"));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let connection = self.db.connection()?;
        connection.execute(
            r#"
            UPDATE agents
            SET name = ?2, instructions = ?3, tools_json = ?4, model = ?5, updated_at = ?6
            WHERE id = ?1
            "#,
            params![
                id,
                name,
                input.instructions,
                serde_json::to_string(&input.tools)?,
                model,
                now,
            ],
        )?;
        self.get(id)?
            .ok_or_else(|| AppError::new("更新 Agent 后未能读取记录"))
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let existing = self
            .get(id)?
            .ok_or_else(|| AppError::new("指定 Agent 不存在"))?;

        if existing.is_system {
            return Err(AppError::new("系统 Agent 不允许删除"));
        }

        let connection = self.db.connection()?;
        connection.execute("DELETE FROM agents WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> AppResult<Option<AgentRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, name, instructions, tools_json, model, status, hooks_json,
                   is_system, created_at, updated_at
            FROM agents WHERE id = ?1 LIMIT 1
            "#,
        )?;

        let agent = statement
            .query_row(params![id], |row| {
                let tools_json: String = row.get(3)?;
                let hooks_json: String = row.get(6)?;
                let status: String = row.get(5)?;
                Ok(AgentRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    instructions: row.get(2)?,
                    tools: serde_json::from_str(&tools_json).unwrap_or_default(),
                    model: row.get(4)?,
                    hooks: serde_json::from_str(&hooks_json).unwrap_or_default(),
                    status: match status.as_str() {
                        "running" => AgentStatus::Running,
                        "stopped" => AgentStatus::Stopped,
                        _ => AgentStatus::Idle,
                    },
                    is_system: row.get::<_, i64>(7).unwrap_or(0) != 0,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })
            .optional()?;

        Ok(agent)
    }

    pub fn set_status(&self, id: &str, status: AgentStatus) -> AppResult<()> {
        let connection = self.db.connection()?;
        connection.execute(
            "UPDATE agents SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, status_string(&status), chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }
}

fn status_string(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Idle => "idle",
        AgentStatus::Running => "running",
        AgentStatus::Stopped => "stopped",
    }
}

fn default_agents() -> Vec<(AgentConfigInput, bool)> {
    vec![
        // ── 系统 Agent: Reviewer ──
        // 只读工具，禁止 write_file / apply_patch / run_shell 等写入和执行类工具
        // 审查场景必须保证零副作用，仅做分析不做修改
        (AgentConfigInput {
            name: "Reviewer".into(),
            instructions: Some("专注于代码审查、问题定位、复杂度分析与风险提示。".into()),
            tools: vec![
                "read_file".into(),
                "search_code".into(),
                "grep_pattern".into(),
                "analyze_ast".into(),
                "check_complexity".into(),
                "find_code_smells".into(),
            ],
            model: "gpt-5.4-mini".into(),
        }, true),
        // ── 系统 Agent: Orchestrator ──
        // 只读工具，禁止 write_file / apply_patch / run_shell 等写入和执行类工具
        // 编排场景只需读取和分析，不应直接修改代码
        (AgentConfigInput {
            name: "Orchestrator".into(),
            instructions: Some("负责拆解任务、分配执行顺序，并汇总多模块结果。".into()),
            tools: vec![
                "read_file".into(),
                "list_directory".into(),
                "search_code".into(),
                "grep_pattern".into(),
                "analyze_ast".into(),
                "find_code_smells".into(),
                "suggest_refactor".into(),
            ],
            model: "gpt-5.4-mini".into(),
        }, true),
        (AgentConfigInput {
            name: "Refactorer".into(),
            instructions: Some("专注于生成结构清晰、风险可控的重构建议与补丁。".into()),
            tools: vec![
                "read_file".into(),
                "write_file".into(),
                "apply_patch".into(),
                "suggest_refactor".into(),
            ],
            model: "gpt-5.4-mini".into(),
        }, false),
        (AgentConfigInput {
            name: "Researcher".into(),
            instructions: Some("负责检索仓库上下文、总结模式，并提供技术背景说明。".into()),
            tools: vec![
                "read_file".into(),
                "list_directory".into(),
                "search_code".into(),
                "grep_pattern".into(),
            ],
            model: "gpt-5.4-mini".into(),
        }, false),
        (AgentConfigInput {
            name: "Executor".into(),
            instructions: Some("负责在隔离工作区执行命令、运行测试并反馈结果。".into()),
            tools: vec!["run_shell".into(), "run_tests".into(), "read_file".into()],
            model: "gpt-5.4-mini".into(),
        }, false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::Database;

    #[test]
    fn creates_default_agent() {
        let db_path =
            std::env::temp_dir().join(format!("codeforge-agent-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(&db_path).expect("db should initialize");
        let store = AgentStore::new(db);
        store
            .ensure_default_agent()
            .expect("default agent should exist");
        let agents = store.list().expect("agents should load");
        assert_eq!(agents.len(), 5);
        let system_count = agents.iter().filter(|a| a.is_system).count();
        assert_eq!(system_count, 2);
    }

    #[test]
    fn system_agent_cannot_be_deleted() {
        let db_path =
            std::env::temp_dir().join(format!("codeforge-agent-del-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(&db_path).expect("db should initialize");
        let store = AgentStore::new(db);
        store.ensure_default_agent().expect("default agent should exist");
        let agents = store.list().expect("agents should load");
        let system = agents.iter().find(|a| a.is_system).unwrap();
        assert!(store.delete(&system.id).is_err());
    }

    #[test]
    fn system_agent_can_be_updated() {
        let db_path =
            std::env::temp_dir().join(format!("codeforge-agent-upd-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(&db_path).expect("db should initialize");
        let store = AgentStore::new(db);
        store.ensure_default_agent().expect("default agent should exist");
        let agents = store.list().expect("agents should load");
        let reviewer = agents.iter().find(|a| a.name == "Reviewer").unwrap();
        let updated = store.update(&reviewer.id, AgentConfigInput {
            name: "Reviewer".into(),
            instructions: Some("自定义审查指令".into()),
            tools: vec!["read_file".into()],
            model: "my-custom-model".into(),
        }).expect("update should succeed");
        assert_eq!(updated.model, "my-custom-model");
        assert_eq!(updated.instructions.as_deref(), Some("自定义审查指令"));
        assert!(updated.is_system);
    }
}
