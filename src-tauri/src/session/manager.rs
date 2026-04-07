use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::sqlite::Database;
use crate::error::{AppError, AppResult};
use crate::llm::model::model_context_window;
use crate::session::logging::record_structured_log;

const DEFAULT_SESSION_TITLE: &str = "新会话";

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

        let rows = statement.query_map([], row_to_session_record)?;
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
        if context_tokens_max == 0 {
            return Err(AppError::new("上下文上限必须大于 0"));
        }

        let connection = self.db.connection()?;
        ensure_agent_exists(&connection, &agent_id)?;

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let normalized_title = normalize_session_title(title);

        let inserted = connection.execute(
            r#"
            INSERT INTO sessions (id, title, agent_id, context_tokens_used, context_tokens_max, created_at, updated_at)
            VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6)
            "#,
            params![
                id,
                normalized_title,
                agent_id,
                context_tokens_max as i64,
                now,
                now
            ],
        )?;

        if inserted == 0 {
            return Err(AppError::new("创建会话失败"));
        }

        let record = SessionRecord {
            id,
            title: normalized_title,
            agent_id,
            context_tokens_used: 0,
            context_tokens_max,
            created_at: now.clone(),
            updated_at: now,
        };

        record_structured_log(
            &self.db,
            "session_create",
            serde_json::json!({
                "sessionId": record.id,
                "agentId": record.agent_id,
                "title": record.title,
                "contextTokensMax": record.context_tokens_max,
            }),
        );

        Ok(record)
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
            .query_row(params![id], row_to_session_record)
            .optional()?;
        Ok(session)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let connection = self.db.connection()?;
        let deleted = connection.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;

        record_structured_log(
            &self.db,
            "session_delete",
            serde_json::json!({
                "sessionId": id,
                "deleted": deleted > 0,
            }),
        );

        Ok(())
    }

    pub fn maybe_auto_rename(&self, session_id: &str, message: &str) -> AppResult<Option<String>> {
        let connection = self.db.connection()?;
        let current_title = connection
            .query_row(
                "SELECT title FROM sessions WHERE id = ?1 LIMIT 1",
                params![session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        let Some(current_title) = current_title else {
            return Err(AppError::new(format!("会话不存在: {session_id}")));
        };

        if !should_auto_title_update(&current_title) {
            return Ok(None);
        }

        let next_title = generate_session_title_from_message(message);
        if next_title == DEFAULT_SESSION_TITLE {
            return Ok(None);
        }

        let updated = connection.execute(
            "UPDATE sessions SET title = ?2, updated_at = ?3 WHERE id = ?1",
            params![session_id, next_title, chrono::Utc::now().to_rfc3339()],
        )?;
        ensure_session_updated(updated, session_id)?;

        record_structured_log(
            &self.db,
            "session_auto_rename",
            serde_json::json!({
                "sessionId": session_id,
                "title": next_title,
            }),
        );

        Ok(Some(next_title))
    }

    pub fn messages(&self, session_id: &str) -> AppResult<Vec<SessionMessage>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, session_id, role, content, tool_calls_json, created_at
            FROM messages
            WHERE session_id = ?1
            ORDER BY created_at ASC, rowid ASC
            "#,
        )?;

        let rows = statement.query_map(params![session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;

        let mut messages = Vec::new();
        for row in rows {
            let (id, message_session_id, role, content, tool_calls_json, created_at) = row?;
            let tool_calls = match serde_json::from_str::<Vec<serde_json::Value>>(&tool_calls_json)
            {
                Ok(value) => value,
                Err(error) => {
                    record_structured_log(
                        &self.db,
                        "session_message_tool_calls_parse_failed",
                        serde_json::json!({
                            "sessionId": session_id,
                            "messageId": id,
                            "error": error.to_string(),
                        }),
                    );
                    Vec::new()
                }
            };

            messages.push(SessionMessage {
                id,
                session_id: message_session_id,
                role,
                content,
                tool_calls,
                created_at,
            });
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
        if role.trim().is_empty() {
            return Err(AppError::new("消息角色不能为空"));
        }

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let serialized_tool_calls = serde_json::to_string(&tool_calls)?;

        let mut connection = self.db.connection()?;
        let transaction = connection.transaction()?;

        let session_exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = ?1)",
            params![session_id],
            |row| row.get::<_, i64>(0),
        )?;
        if session_exists == 0 {
            return Err(AppError::new(format!("会话不存在: {session_id}")));
        }

        transaction.execute(
            r#"
            INSERT INTO messages (id, session_id, role, content, tool_calls_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![id, session_id, role, content, serialized_tool_calls, now],
        )?;

        let updated = transaction.execute(
            "UPDATE sessions SET updated_at = ?2 WHERE id = ?1",
            params![session_id, chrono::Utc::now().to_rfc3339()],
        )?;
        ensure_session_updated(updated, session_id)?;

        transaction.commit()?;

        record_structured_log(
            &self.db,
            "session_append_message",
            serde_json::json!({
                "sessionId": session_id,
                "messageId": id,
                "role": role,
                "contentLength": content.chars().count(),
                "toolCallCount": tool_calls.len(),
            }),
        );

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
        let updated = connection.execute(
            "UPDATE sessions SET context_tokens_used = ?2, updated_at = ?3 WHERE id = ?1",
            params![
                session_id,
                used_tokens as i64,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;

        ensure_session_updated(updated, session_id)?;

        record_structured_log(
            &self.db,
            "session_update_usage",
            serde_json::json!({
                "sessionId": session_id,
                "contextTokensUsed": used_tokens,
            }),
        );

        Ok(())
    }

    pub fn update_context_max(&self, session_id: &str, context_tokens_max: usize) -> AppResult<()> {
        if context_tokens_max == 0 {
            return Err(AppError::new("上下文上限必须大于 0"));
        }

        let connection = self.db.connection()?;
        let updated = connection.execute(
            "UPDATE sessions SET context_tokens_max = ?2, updated_at = ?3 WHERE id = ?1",
            params![
                session_id,
                context_tokens_max as i64,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;

        ensure_session_updated(updated, session_id)?;

        record_structured_log(
            &self.db,
            "session_update_context_max",
            serde_json::json!({
                "sessionId": session_id,
                "contextTokensMax": context_tokens_max,
            }),
        );

        Ok(())
    }

    pub fn normalize_model_context_max(&self, session_id: &str, model: &str) -> AppResult<()> {
        let context_tokens_max = model_context_window(model);
        self.update_context_max(session_id, context_tokens_max)?;

        record_structured_log(
            &self.db,
            "session_normalize_context_max",
            serde_json::json!({
                "sessionId": session_id,
                "model": model,
                "contextTokensMax": context_tokens_max,
            }),
        );

        Ok(())
    }
}

fn row_to_session_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRecord> {
    Ok(SessionRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        agent_id: row.get(2)?,
        context_tokens_used: row.get::<_, i64>(3)? as usize,
        context_tokens_max: row.get::<_, i64>(4)? as usize,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn normalize_session_title(title: Option<String>) -> String {
    title
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or_else(|| DEFAULT_SESSION_TITLE.to_string())
}

fn should_auto_title_update(title: &str) -> bool {
    let trimmed = title.trim();
    trimmed.is_empty() || trimmed == DEFAULT_SESSION_TITLE
}

fn generate_session_title_from_message(message: &str) -> String {
    let sanitized = message
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let mut seed = sanitized.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\''
                | '`'
                | '['
                | ']'
                | '('
                | ')'
                | '{'
                | '}'
                | '。'
                | '！'
                | '!'
                | '？'
                | '?'
                | '，'
                | ','
                | '：'
                | ':'
        )
    });

    if let Some(index) = seed.find(['。', '！', '？', '!', '?', ';', '；']) {
        seed = &seed[..index];
    }

    if seed.is_empty() {
        return DEFAULT_SESSION_TITLE.to_string();
    }

    if contains_cjk(seed) {
        let title = seed.chars().take(18).collect::<String>().trim().to_string();
        if title.is_empty() {
            DEFAULT_SESSION_TITLE.to_string()
        } else {
            title
        }
    } else {
        let mut title = seed
            .split_whitespace()
            .take(8)
            .collect::<Vec<_>>()
            .join(" ");
        if title.chars().count() > 48 {
            title = title.chars().take(48).collect();
        }
        let trimmed = title.trim().to_string();
        if trimmed.is_empty() {
            DEFAULT_SESSION_TITLE.to_string()
        } else {
            trimmed
        }
    }
}

fn contains_cjk(value: &str) -> bool {
    value.chars().any(|ch| {
        ('\u{3400}'..='\u{4dbf}').contains(&ch)
            || ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ('\u{f900}'..='\u{faff}').contains(&ch)
    })
}

fn ensure_agent_exists(connection: &rusqlite::Connection, agent_id: &str) -> AppResult<()> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM agents WHERE id = ?1)",
        params![agent_id],
        |row| row.get::<_, i64>(0),
    )?;
    if exists == 0 {
        return Err(AppError::new(format!("Agent 不存在: {agent_id}")));
    }
    Ok(())
}

fn ensure_session_updated(updated_rows: usize, session_id: &str) -> AppResult<()> {
    if updated_rows == 0 {
        return Err(AppError::new(format!("会话不存在: {session_id}")));
    }
    Ok(())
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

    #[test]
    fn auto_title_from_first_message_uses_short_summary() {
        let title =
            generate_session_title_from_message("请帮我查看当前目录有什么内容，并按文件类型分类");
        assert!(!title.is_empty());
        assert!(title.chars().count() <= 18);
    }
}
