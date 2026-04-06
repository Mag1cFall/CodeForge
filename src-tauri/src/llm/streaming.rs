use futures::StreamExt;

use crate::error::AppResult;

pub async fn consume_sse_chunks<F, T>(
    response: reqwest::Response,
    mut parse: F,
) -> AppResult<Vec<T>>
where
    F: FnMut(serde_json::Value) -> Option<T>,
{
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut items = Vec::new();

    while let Some(item) = stream.next().await {
        let bytes = item?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(position) = buffer.find('\n') {
            let line = buffer.drain(..=position).collect::<String>();
            let line = line.trim();
            if !line.starts_with("data:") {
                continue;
            }

            let payload = line.trim_start_matches("data:").trim();
            if payload.is_empty() || payload == "[DONE]" {
                continue;
            }

            let value: serde_json::Value = serde_json::from_str(payload)?;
            if let Some(parsed) = parse(value) {
                items.push(parsed);
            }
        }
    }

    Ok(items)
}
