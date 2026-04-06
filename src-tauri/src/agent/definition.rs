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

        let _ = self.create(AgentConfigInput {
            name: "Orchestrator".into(),
            instructions: Some(
                "You are CodeForge's orchestrator agent for code analysis and review.".into(),
            ),
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
        })?;
        Ok(())
    }

    pub fn list(&self) -> AppResult<Vec<AgentRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, name, instructions, tools_json, model, status, hooks_json, created_at, updated_at
            FROM agents ORDER BY updated_at DESC
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
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;

        let mut agents = Vec::new();
        for row in rows {
            agents.push(row?);
        }
        Ok(agents)
    }

    pub fn create(&self, input: AgentConfigInput) -> AppResult<AgentRecord> {
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
            INSERT INTO agents (id, name, instructions, tools_json, model, status, hooks_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, 'idle', ?6, ?7, ?8)
            "#,
            params![
                id,
                name,
                input.instructions,
                serde_json::to_string(&input.tools)?,
                model,
                serde_json::to_string(&AgentHooksConfig::default())?,
                now,
                now,
            ],
        )?;
        self.get(&id)?
            .ok_or_else(|| AppError::new("新建 Agent 后未能读取记录"))
    }

    pub fn get(&self, id: &str) -> AppResult<Option<AgentRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, name, instructions, tools_json, model, status, hooks_json, created_at, updated_at
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
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
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
        assert_eq!(store.list().expect("agents should load").len(), 1);
    }
}
