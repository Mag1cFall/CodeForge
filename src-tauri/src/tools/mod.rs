pub mod analysis_tools;
pub mod file_tools;
pub mod registry;
pub mod schema;
pub mod search_tools;
pub mod shell_tools;

pub(crate) fn emit_structured_log(scope: &str, event: &str, payload: serde_json::Value) {
    let record = serde_json::json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "component": "tools",
        "scope": scope,
        "event": event,
        "payload": payload,
    });
    eprintln!("{}", record);
}
