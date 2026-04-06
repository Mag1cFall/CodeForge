use rusqlite::{params, OptionalExtension};

use crate::db::sqlite::Database;
use crate::error::AppResult;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceLogRecord {
    pub id: i64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceLogFilter {
    pub kind: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct TraceLogService {
    db: Database,
}

impl TraceLogService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn record(&self, kind: &str, payload: serde_json::Value) -> AppResult<()> {
        let sanitized = sanitize_json(payload);
        self.db.append_log(
            kind,
            &serde_json::to_string(&sanitized)?,
            &chrono::Utc::now().to_rfc3339(),
        )
    }

    pub fn list(&self, filter: TraceLogFilter) -> AppResult<Vec<TraceLogRecord>> {
        let connection = self.db.connection()?;
        let limit = filter.limit.unwrap_or(100) as i64;
        if let Some(kind) = filter.kind {
            let mut statement = connection.prepare(
                "SELECT id, kind, payload_json, created_at FROM logs WHERE kind = ?1 ORDER BY id DESC LIMIT ?2",
            )?;
            let rows = statement.query_map(params![kind, limit], row_to_record)?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            return Ok(items);
        } else {
            let mut statement = connection.prepare(
                "SELECT id, kind, payload_json, created_at FROM logs ORDER BY id DESC LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit], row_to_record)?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            return Ok(items);
        }
    }

    pub fn latest(&self, kind: &str) -> AppResult<Option<TraceLogRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, kind, payload_json, created_at FROM logs WHERE kind = ?1 ORDER BY id DESC LIMIT 1",
        )?;
        statement
            .query_row(params![kind], row_to_record)
            .optional()
            .map_err(Into::into)
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<TraceLogRecord> {
    let payload_json: String = row.get(2)?;
    Ok(TraceLogRecord {
        id: row.get(0)?,
        kind: row.get(1)?,
        payload: serde_json::from_str(&payload_json).unwrap_or_else(|_| serde_json::json!({})),
        created_at: row.get(3)?,
    })
}

fn sanitize_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let lower = key.to_ascii_lowercase();
                    if lower.contains("api_key")
                        || lower.contains("apikey")
                        || lower.contains("authorization")
                        || lower.contains("token")
                    {
                        return (
                            key,
                            serde_json::Value::String(mask_secret(value.as_str().unwrap_or(""))),
                        );
                    }
                    (key, sanitize_json(value))
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sanitize_json).collect())
        }
        serde_json::Value::String(text) => serde_json::Value::String(mask_inline_secret(&text)),
        other => other,
    }
}

fn mask_inline_secret(value: &str) -> String {
    if value.starts_with("sk-") {
        return mask_secret(value);
    }
    value.to_string()
}

fn mask_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "missing".into();
    }
    if trimmed.len() <= 6 {
        return format!("{}...{}", &trimmed[..1], &trimmed[trimmed.len() - 1..]);
    }
    if trimmed.len() <= 16 {
        return format!("{}...{}", &trimmed[..2], &trimmed[trimmed.len() - 2..]);
    }
    format!("{}...{}", &trimmed[..8], &trimmed[trimmed.len() - 8..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::Database;

    #[test]
    fn records_and_masks_sensitive_fields() {
        let db_path =
            std::env::temp_dir().join(format!("codeforge-log-{}.db", uuid::Uuid::new_v4()));
        let service = TraceLogService::new(Database::new(&db_path).expect("db should initialize"));
        service
            .record(
                "provider_create",
                serde_json::json!({ "apiKey": "sk-abcdefghijklmnopqrstuvwxyz", "name": "demo" }),
            )
            .expect("log should record");

        let record = service
            .latest("provider_create")
            .expect("latest log should load")
            .expect("latest log should exist");
        assert_eq!(record.payload["apiKey"], "sk-abcde...stuvwxyz");
    }
}
