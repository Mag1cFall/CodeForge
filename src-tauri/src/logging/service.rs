use regex::{Captures, Regex};
use rusqlite::{params, OptionalExtension};
use std::sync::OnceLock;

use crate::db::sqlite::Database;
use crate::error::AppResult;

const REDACT_MIN_LENGTH: usize = 18;
const REDACT_KEEP_START: usize = 6;
const REDACT_KEEP_END: usize = 4;
const REDACT_REGEX_CHUNK_THRESHOLD: usize = 32_768;
const REDACT_REGEX_CHUNK_SIZE: usize = 16_384;

const DEFAULT_REDACT_PATTERNS: [&str; 17] = [
    r#"\b[A-Z0-9_]*(?:KEY|TOKEN|SECRET|PASSWORD|PASSWD)\b\s*[=:]\s*["']?([^\s"'\\]+)["']?"#,
    r#""(?:apiKey|token|secret|password|passwd|accessToken|refreshToken)"\s*:\s*"([^"]+)""#,
    r#"--(?:api[-_]?key|token|secret|password|passwd)\s+["']?([^\s"']+)["']?"#,
    r#"Authorization\s*[:=]\s*Bearer\s+([A-Za-z0-9._\-+=]{18,})"#,
    r#"\bBearer\s+([A-Za-z0-9._\-+=]{18,})\b"#,
    r#"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]+?-----END [A-Z ]*PRIVATE KEY-----"#,
    r#"\b(sk-[A-Za-z0-9_-]{8,})\b"#,
    r#"\b(ghp_[A-Za-z0-9]{20,})\b"#,
    r#"\b(github_pat_[A-Za-z0-9_]{20,})\b"#,
    r#"\b(xox[baprs]-[A-Za-z0-9-]{10,})\b"#,
    r#"\b(xapp-[A-Za-z0-9-]{10,})\b"#,
    r#"\b(gsk_[A-Za-z0-9_-]{10,})\b"#,
    r#"\b(AIza[0-9A-Za-z\-_]{20,})\b"#,
    r#"\b(pplx-[A-Za-z0-9_-]{10,})\b"#,
    r#"\b(npm_[A-Za-z0-9]{10,})\b"#,
    r#"\bbot(\d{6,}:[A-Za-z0-9_-]{20,})\b"#,
    r#"\b(\d{6,}:[A-Za-z0-9_-]{20,})\b"#,
];

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
    let payload = serde_json::from_str::<serde_json::Value>(&payload_json)
        .unwrap_or_else(|_| serde_json::Value::String(payload_json));
    Ok(TraceLogRecord {
        id: row.get(0)?,
        kind: row.get(1)?,
        payload: sanitize_json(payload),
        created_at: row.get(3)?,
    })
}

fn sanitize_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    if is_sensitive_key(&key) {
                        (key, redact_sensitive_value(value))
                    } else {
                        (key, sanitize_json(value))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sanitize_json).collect())
        }
        serde_json::Value::String(text) => serde_json::Value::String(redact_sensitive_text(&text)),
        other => other,
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();

    if normalized.is_empty() {
        return false;
    }

    matches!(
        normalized.as_str(),
        "apikey"
            | "token"
            | "secret"
            | "password"
            | "passwd"
            | "accesstoken"
            | "refreshtoken"
            | "authorization"
    ) || normalized.ends_with("apikey")
        || normalized.ends_with("accesstoken")
        || normalized.ends_with("refreshtoken")
        || normalized.ends_with("authorization")
        || normalized.ends_with("password")
        || normalized.ends_with("passwd")
        || normalized.ends_with("secret")
        || (normalized.ends_with("token")
            && !normalized.ends_with("tokens")
            && !normalized.ends_with("tokencount"))
}

fn redact_sensitive_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => {
            if is_already_masked(&text) {
                return serde_json::Value::String(text);
            }
            let redacted = redact_sensitive_text(&text);
            if redacted != text {
                serde_json::Value::String(redacted)
            } else {
                serde_json::Value::String(mask_token(&text))
            }
        }
        _ => serde_json::Value::String("***".to_string()),
    }
}

fn is_already_masked(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && (trimmed == "***" || trimmed.contains('…'))
}

fn redact_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS
        .get_or_init(|| {
            DEFAULT_REDACT_PATTERNS
                .iter()
                .map(|pattern| Regex::new(pattern).expect("redaction pattern must compile"))
                .collect()
        })
        .as_slice()
}

fn redact_sensitive_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut redacted = text.to_string();
    for pattern in redact_patterns() {
        redacted = replace_pattern_bounded(&redacted, pattern);
    }
    redacted
}

fn replace_pattern_bounded(text: &str, pattern: &Regex) -> String {
    if text.len() <= REDACT_REGEX_CHUNK_THRESHOLD {
        return apply_pattern(text, pattern);
    }

    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        let end = next_chunk_boundary(text, index, REDACT_REGEX_CHUNK_SIZE);
        output.push_str(&apply_pattern(&text[index..end], pattern));
        index = end;
    }
    output
}

fn next_chunk_boundary(text: &str, start: usize, chunk_size: usize) -> usize {
    let mut end = (start + chunk_size).min(text.len());
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end == start {
        if let Some(ch) = text[start..].chars().next() {
            return start + ch.len_utf8();
        }
        return text.len();
    }
    end
}

fn apply_pattern(text: &str, pattern: &Regex) -> String {
    pattern
        .replace_all(text, |captures: &Captures<'_>| redact_match(captures))
        .into_owned()
}

fn redact_match(captures: &Captures<'_>) -> String {
    let matched = captures
        .get(0)
        .map(|item| item.as_str())
        .unwrap_or_default();

    if matched.contains("PRIVATE KEY-----") {
        return redact_pem_block(matched);
    }

    let token = captures
        .iter()
        .skip(1)
        .flatten()
        .map(|item| item.as_str())
        .filter(|value| !value.is_empty())
        .last()
        .unwrap_or(matched);

    let masked = mask_token(token);
    if token == matched {
        return masked;
    }
    matched.replacen(token, &masked, 1)
}

fn redact_pem_block(block: &str) -> String {
    let lines: Vec<&str> = block
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.len() < 2 {
        return "***".to_string();
    }
    format!("{}\n…redacted…\n{}", lines[0], lines[lines.len() - 1])
}

fn mask_token(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "***".to_string();
    }
    if is_already_masked(trimmed) {
        return trimmed.to_string();
    }
    if trimmed.chars().count() < REDACT_MIN_LENGTH {
        return "***".to_string();
    }

    let start = trimmed.chars().take(REDACT_KEEP_START).collect::<String>();
    let end = trimmed
        .chars()
        .rev()
        .take(REDACT_KEEP_END)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}…{end}")
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
                serde_json::json!({
                    "apiKey": "sk-abcdefghijklmnopqrstuvwxyz",
                    "tokenCount": 128,
                    "name": "demo"
                }),
            )
            .expect("log should record");

        let record = service
            .latest("provider_create")
            .expect("latest log should load")
            .expect("latest log should exist");
        assert_eq!(record.payload["apiKey"], "sk-abc…wxyz");
        assert_eq!(record.payload["tokenCount"], 128);
    }

    #[test]
    fn redacts_bearer_token_inside_text_field() {
        let db_path =
            std::env::temp_dir().join(format!("codeforge-log-{}.db", uuid::Uuid::new_v4()));
        let service = TraceLogService::new(Database::new(&db_path).expect("db should initialize"));
        service
            .record(
                "tool",
                serde_json::json!({
                    "detail": "Authorization: Bearer abcdef1234567890ghij"
                }),
            )
            .expect("log should record");

        let record = service
            .latest("tool")
            .expect("latest log should load")
            .expect("latest log should exist");
        assert_eq!(
            record.payload["detail"],
            "Authorization: Bearer abcdef…ghij"
        );
    }

    #[test]
    fn redacts_private_key_blocks() {
        let db_path =
            std::env::temp_dir().join(format!("codeforge-log-{}.db", uuid::Uuid::new_v4()));
        let service = TraceLogService::new(Database::new(&db_path).expect("db should initialize"));
        let private_key = [
            "-----BEGIN PRIVATE KEY-----",
            "ABCDEF1234567890",
            "ZYXWVUT987654321",
            "-----END PRIVATE KEY-----",
        ]
        .join("\n");

        service
            .record("tool", serde_json::json!({ "secret": private_key }))
            .expect("log should record");

        let record = service
            .latest("tool")
            .expect("latest log should load")
            .expect("latest log should exist");
        assert_eq!(
            record.payload["secret"],
            "-----BEGIN PRIVATE KEY-----\n…redacted…\n-----END PRIVATE KEY-----"
        );
    }

    #[test]
    fn redacts_invalid_json_payload_when_loading() {
        let db_path =
            std::env::temp_dir().join(format!("codeforge-log-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(&db_path).expect("db should initialize");
        db.append_log(
            "legacy",
            "Authorization: Bearer abcdef1234567890ghij",
            &chrono::Utc::now().to_rfc3339(),
        )
        .expect("legacy payload should insert");

        let service = TraceLogService::new(db);
        let record = service
            .latest("legacy")
            .expect("latest log should load")
            .expect("latest log should exist");
        assert_eq!(
            record.payload,
            serde_json::Value::String("Authorization: Bearer abcdef…ghij".to_string())
        );
    }
}
