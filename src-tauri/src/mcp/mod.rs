pub mod client;
pub mod server_mgr;
pub mod transport;

pub(crate) fn log_structured(scope: &str, event: &str, payload: serde_json::Value) {
    let log = serde_json::json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "scope": scope,
        "event": event,
        "payload": payload,
    });
    eprintln!("{log}");
}
