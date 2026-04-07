use futures::StreamExt;

use crate::error::AppResult;

use super::telemetry::log_event;

#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event: Option<String>,
    pub payload: serde_json::Value,
}

pub async fn consume_sse_events(response: reqwest::Response) -> AppResult<(Vec<SseEvent>, bool)> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut current_event: Option<String> = None;
    let mut data_lines: Vec<String> = Vec::new();
    let mut events = Vec::new();
    let mut done = false;
    let mut parse_errors = 0_usize;

    while let Some(item) = stream.next().await {
        let bytes = item?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(position) = buffer.find('\n') {
            let mut line = buffer.drain(..=position).collect::<String>();
            if line.ends_with('\n') {
                line.pop();
            }
            if line.ends_with('\r') {
                line.pop();
            }

            if line.is_empty() {
                if !data_lines.is_empty() {
                    flush_data_block(
                        &mut events,
                        &mut done,
                        &mut parse_errors,
                        &mut current_event,
                        &mut data_lines,
                    );
                    data_lines.clear();
                }
                current_event = None;
                continue;
            }

            if line.starts_with(':') {
                continue;
            }

            if let Some(value) = line.strip_prefix("event:") {
                current_event = Some(value.trim().to_string());
                continue;
            }
            if let Some(value) = line.strip_prefix("data:") {
                data_lines.push(value.trim_start().to_string());
            }
        }
    }

    if !data_lines.is_empty() {
        flush_data_block(
            &mut events,
            &mut done,
            &mut parse_errors,
            &mut current_event,
            &mut data_lines,
        );
    }

    if parse_errors > 0 {
        log_event(
            "sse",
            "parse_error_summary",
            serde_json::json!({
                "errors": parse_errors,
                "eventsAccepted": events.len(),
            }),
        );
    }

    log_event(
        "sse",
        "consume_completed",
        serde_json::json!({
            "events": events.len(),
            "done": done,
        }),
    );

    Ok((events, done))
}

fn flush_data_block(
    events: &mut Vec<SseEvent>,
    done: &mut bool,
    parse_errors: &mut usize,
    current_event: &mut Option<String>,
    data_lines: &mut Vec<String>,
) {
    let payload = data_lines.join("\n");
    if payload.trim() == "[DONE]" {
        *done = true;
        return;
    }

    if payload.trim().is_empty() {
        return;
    }

    match serde_json::from_str::<serde_json::Value>(&payload) {
        Ok(parsed) => {
            events.push(SseEvent {
                event: current_event.take(),
                payload: parsed,
            });
        }
        Err(error) => {
            *parse_errors += 1;
            log_event(
                "sse",
                "parse_error",
                serde_json::json!({
                    "event": current_event.clone(),
                    "error": error.to_string(),
                    "payloadPreview": truncate_for_log(&payload),
                }),
            );
        }
    }
}

fn truncate_for_log(value: &str) -> String {
    const LIMIT: usize = 512;
    if value.len() <= LIMIT {
        return value.to_string();
    }
    format!("{}...(truncated)", &value[..LIMIT])
}
