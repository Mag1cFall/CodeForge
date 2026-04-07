pub fn log_event(component: &str, event: &str, payload: serde_json::Value) {
    let sanitized = sanitize_json(payload);
    let entry = serde_json::json!({
        "scope": "llm",
        "component": component,
        "event": event,
        "ts": chrono::Utc::now().to_rfc3339(),
        "payload": sanitized,
    });

    if let Ok(text) = serde_json::to_string(&entry) {
        eprintln!("{text}");
    } else {
        eprintln!(
            "{{\"scope\":\"llm\",\"component\":\"{}\",\"event\":\"{}\"}}",
            component, event
        );
    }
}

fn sanitize_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    if should_mask_key(&key) {
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

fn should_mask_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    lowered.contains("api_key")
        || lowered.contains("apikey")
        || lowered.contains("authorization")
        || lowered.contains("token")
        || lowered.contains("secret")
}

fn mask_inline_secret(value: &str) -> String {
    if value.trim().starts_with("sk-") || value.trim().starts_with("Bearer ") {
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
