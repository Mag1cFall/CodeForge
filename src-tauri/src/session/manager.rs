use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::sqlite::Database;
use crate::error::{AppError, AppResult};
use crate::llm::model::model_context_window;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub agent_id: String,
    pub context_tokens_used: usize,
    pub context_tokens_max: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SessionManager {
    db: Database,
}

impl SessionManager {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list(&self) -> AppResult<Vec<SessionRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, title, agent_id, context_tokens_used, context_tokens_max, created_at, updated_at
            FROM sessions
            ORDER BY updated_at DESC
            "#,
        )?;

        let rows = statement.query_map([], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                agent_id: row.get(2)?,
                context_tokens_used: row.get::<_, i64>(3)? as usize,
                context_tokens_max: row.get::<_, i64>(4)? as usize,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        Ok(sessions)
    }

    pub fn create(&self, agent_id: String, title: Option<String>) -> AppResult<SessionRecord> {
        self.create_with_context_max(agent_id, title, 128_000)
    }

    pub fn create_with_context_max(
        &self,
        agent_id: String,
        title: Option<String>,
        context_tokens_max: usize,
    ) -> AppResult<SessionRecord> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let title = title.unwrap_or_else(|| "新会话".into());

        let connection = self.db.connection()?;
        connection.execute(
            r#"
            INSERT INTO sessions (id, title, agent_id, context_tokens_used, context_tokens_max, created_at, updated_at)
            VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6)
            "#,
            params![id, title, agent_id, context_tokens_max as i64, now, now],
        )?;

        self.get(&id)?
            .ok_or_else(|| AppError::new("创建会话后未能读取结果"))
    }

    pub fn get(&self, id: &str) -> AppResult<Option<SessionRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, title, agent_id, context_tokens_used, context_tokens_max, created_at, updated_at
            FROM sessions WHERE id = ?1 LIMIT 1
            "#,
        )?;
        let session = statement
            .query_row(params![id], |row| {
                Ok(SessionRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    agent_id: row.get(2)?,
                    context_tokens_used: row.get::<_, i64>(3)? as usize,
                    context_tokens_max: row.get::<_, i64>(4)? as usize,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .optional()?;
        Ok(session)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let connection = self.db.connection()?;
        connection.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn messages(&self, session_id: &str) -> AppResult<Vec<SessionMessage>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, session_id, role, content, tool_calls_json, created_at
            FROM messages WHERE session_id = ?1 ORDER BY created_at ASC
            "#,
        )?;

        let rows = statement.query_map(params![session_id], |row| {
            let tool_calls_json: String = row.get(4)?;
            Ok(SessionMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                tool_calls: serde_json::from_str(&tool_calls_json).unwrap_or_default(),
                created_at: row.get(5)?,
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    pub fn append_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        tool_calls: Vec<serde_json::Value>,
    ) -> AppResult<SessionMessage> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let connection = self.db.connection()?;
        connection.execute(
            r#"
            INSERT INTO messages (id, session_id, role, content, tool_calls_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                id,
                session_id,
                role,
                content,
                serde_json::to_string(&tool_calls)?,
                now
            ],
        )?;
        connection.execute(
            "UPDATE sessions SET updated_at = ?2 WHERE id = ?1",
            params![session_id, chrono::Utc::now().to_rfc3339()],
        )?;

        Ok(SessionMessage {
            id,
            session_id: session_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            tool_calls,
            created_at: now,
        })
    }

    pub fn update_usage(&self, session_id: &str, used_tokens: usize) -> AppResult<()> {
        let connection = self.db.connection()?;
        connection.execute(
            "UPDATE sessions SET context_tokens_used = ?2, updated_at = ?3 WHERE id = ?1",
            params![
                session_id,
                used_tokens as i64,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn update_context_max(&self, session_id: &str, context_tokens_max: usize) -> AppResult<()> {
        let connection = self.db.connection()?;
        connection.execute(
            "UPDATE sessions SET context_tokens_max = ?2, updated_at = ?3 WHERE id = ?1",
            params![
                session_id,
                context_tokens_max as i64,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn normalize_model_context_max(&self, session_id: &str, model: &str) -> AppResult<()> {
        self.update_context_max(session_id, model_context_window(model))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::Database;

    #[test]
    fn creates_and_reads_session_messages() {
        let db_path =
            std::env::temp_dir().join(format!("codeforge-session-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(&db_path).expect("db should initialize");

        let connection = db.connection().expect("db connection should open");
        connection
            .execute(
                r#"
                INSERT INTO agents (id, name, instructions, tools_json, model, status, hooks_json, created_at, updated_at)
                VALUES ('agent-1', 'Main', '', '[]', 'gpt-5.4-mini', 'idle', '{}', ?1, ?1)
                "#,
                params![chrono::Utc::now().to_rfc3339()],
            )
            .expect("agent seed should succeed");

        let manager = SessionManager::new(db);
        let session = manager
            .create("agent-1".into(), Some("测试会话".into()))
            .expect("session should be created");
        manager
            .append_message(&session.id, "user", "hello", vec![])
            .expect("message should be appended");

        let messages = manager
            .messages(&session.id)
            .expect("messages should be available");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "hello");
    }
}
