use futures::StreamExt;

use crate::error::AppResult;

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
                    let payload = data_lines.join("\n");
                    if payload.trim() == "[DONE]" {
                        done = true;
                    } else if !payload.trim().is_empty() {
                        events.push(SseEvent {
                            event: current_event.take(),
                            payload: serde_json::from_str(&payload)?,
                        });
                    }
                    data_lines.clear();
                }
                current_event = None;
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
        let payload = data_lines.join("\n");
        if payload.trim() == "[DONE]" {
            done = true;
        } else if !payload.trim().is_empty() {
            events.push(SseEvent {
                event: current_event,
                payload: serde_json::from_str(&payload)?,
            });
        }
    }

    Ok((events, done))
}
