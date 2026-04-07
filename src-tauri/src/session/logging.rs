use chrono::Utc;

use crate::db::sqlite::Database;

pub(crate) fn record_structured_log(db: &Database, kind: &str, payload: serde_json::Value) {
    if let Ok(payload_json) = serde_json::to_string(&payload) {
        let _ = db.append_log(kind, &payload_json, &Utc::now().to_rfc3339());
    }
}
