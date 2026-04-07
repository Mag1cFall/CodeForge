pub mod embedder;
pub mod indexer;
pub mod retriever;
pub mod store;

pub(crate) fn knowledge_log(event: &str, payload: serde_json::Value) {
    let line = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "scope": "knowledge",
        "event": event,
        "payload": payload,
    });
    eprintln!("{}", line);
}
