use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::StatusCode;

use crate::error::{AppError, AppResult};

use super::knowledge_log;

const DEFAULT_EMBEDDING_PROVIDER: &str = "openai";
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_EMBEDDING_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_TIMEOUT_SECS: u64 = 20;
const DEFAULT_MAX_RETRIES: usize = 3;
const DEFAULT_MAX_INPUT_BYTES: usize = 8192;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedderInfo {
    pub provider: String,
    pub model: String,
    pub max_input_bytes: usize,
}

pub trait Embedder: Send + Sync {
    fn embed(&self, text: &str) -> AppResult<Vec<f32>>;
    fn info(&self) -> EmbedderInfo;
}

#[derive(Debug, Clone)]
pub struct ApiEmbedder {
    client: Client,
    provider: String,
    base_url: String,
    api_key: String,
    model: String,
    dimensions: Option<usize>,
    max_input_bytes: usize,
    max_retries: usize,
}

impl ApiEmbedder {
    pub fn from_env() -> AppResult<Option<Self>> {
        let provider = std::env::var("EMBEDDING_PROVIDER")
            .unwrap_or_else(|_| DEFAULT_EMBEDDING_PROVIDER.to_string())
            .trim()
            .to_ascii_lowercase();
        if provider == "none" {
            knowledge_log(
                "embedder.disabled",
                serde_json::json!({ "reason": "EMBEDDING_PROVIDER=none" }),
            );
            return Ok(None);
        }

        let api_key = read_env_first(&["EMBEDDING_API_KEY", "OPENAI_API_KEY"]).unwrap_or_default();
        if api_key.is_empty() {
            knowledge_log(
                "embedder.unavailable",
                serde_json::json!({ "reason": "missing embedding api key", "provider": provider }),
            );
            return Ok(None);
        }

        let raw_base = read_env_first(&["EMBEDDING_API_BASE", "OPENAI_API_BASE"])
            .unwrap_or_else(|| DEFAULT_EMBEDDING_BASE.to_string());
        let model = normalize_model(
            read_env_first(&["EMBEDDING_MODEL"])
                .as_deref()
                .unwrap_or(DEFAULT_EMBEDDING_MODEL),
        );
        let dimensions = read_env_usize("EMBEDDING_DIMENSIONS");
        let timeout_secs = read_env_u64("EMBEDDING_TIMEOUT_SECS").unwrap_or(DEFAULT_TIMEOUT_SECS);
        let max_retries = read_env_usize("EMBEDDING_MAX_RETRIES")
            .unwrap_or(DEFAULT_MAX_RETRIES)
            .max(1);
        let max_input_bytes = read_env_usize("EMBEDDING_MAX_INPUT_BYTES")
            .unwrap_or_else(|| known_max_input_bytes(&provider, &model))
            .max(256);

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs.max(1)))
            .build()
            .map_err(|error| AppError::new(error.to_string()))?;

        let embedder = Self {
            client,
            provider,
            base_url: normalize_base_url(&raw_base),
            api_key,
            model,
            dimensions,
            max_input_bytes,
            max_retries,
        };

        let info = embedder.info();
        knowledge_log(
            "embedder.ready",
            serde_json::json!({
                "provider": info.provider,
                "model": info.model,
                "maxInputBytes": info.max_input_bytes,
                "maxRetries": embedder.max_retries,
            }),
        );

        Ok(Some(embedder))
    }

    fn embed_single(&self, text: &str) -> AppResult<Vec<f32>> {
        let mut last_error: Option<AppError> = None;
        let request_body = self.build_request_body(text);
        let endpoint = format!("{}/embeddings", self.base_url.trim_end_matches('/'));

        for attempt in 1..=self.max_retries {
            if attempt > 1 {
                std::thread::sleep(retry_delay(attempt - 1));
            }

            let response = self
                .client
                .post(&endpoint)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send();

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(AppError::new(error.to_string()));
                    continue;
                }
            };

            let status = response.status();
            if !status.is_success() {
                let body_text = response.text().unwrap_or_default();
                let message = format!(
                    "embedding request failed: status={}, body={}",
                    status,
                    truncate_for_log(&body_text, 256)
                );
                if is_retryable_status(status) {
                    last_error = Some(AppError::new(message));
                    continue;
                }
                return Err(AppError::new(message));
            }

            let payload: serde_json::Value = response
                .json()
                .map_err(|error| AppError::new(error.to_string()))?;
            return parse_embedding_response(payload);
        }

        Err(last_error.unwrap_or_else(|| AppError::new("embedding request failed")))
    }

    fn build_request_body(&self, text: &str) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.model,
            "input": text,
        });
        if let Some(dimensions) = self.dimensions {
            body["dimensions"] = serde_json::json!(dimensions);
        }
        body
    }
}

impl Embedder for ApiEmbedder {
    fn embed(&self, text: &str) -> AppResult<Vec<f32>> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let segments = split_text_to_utf8_byte_limit(trimmed, self.max_input_bytes);
        if segments.len() <= 1 {
            return self.embed_single(trimmed);
        }

        knowledge_log(
            "embedder.input.split",
            serde_json::json!({
                "provider": self.provider,
                "model": self.model,
                "segments": segments.len(),
                "bytes": trimmed.len(),
                "maxInputBytes": self.max_input_bytes,
            }),
        );

        let mut merged = Vec::<f32>::new();
        let mut count = 0usize;
        for segment in segments {
            let embedding = self.embed_single(&segment)?;
            if embedding.is_empty() {
                continue;
            }
            if merged.is_empty() {
                merged = vec![0.0; embedding.len()];
            }
            if merged.len() != embedding.len() {
                return Err(AppError::new(
                    "embedding dimension mismatch while merging segmented input",
                ));
            }
            for (acc, value) in merged.iter_mut().zip(embedding.iter()) {
                *acc += *value;
            }
            count += 1;
        }

        if count == 0 {
            return Ok(Vec::new());
        }
        for value in &mut merged {
            *value /= count as f32;
        }
        Ok(sanitize_and_normalize_embedding(merged))
    }

    fn info(&self) -> EmbedderInfo {
        EmbedderInfo {
            provider: self.provider.clone(),
            model: self.model.clone(),
            max_input_bytes: self.max_input_bytes,
        }
    }
}

pub fn create_embedder() -> AppResult<Option<Box<dyn Embedder>>> {
    Ok(ApiEmbedder::from_env()?.map(|embedder| Box::new(embedder) as Box<dyn Embedder>))
}

fn read_env_first(keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn read_env_u64(key: &str) -> Option<u64> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
}

fn normalize_base_url(raw: &str) -> String {
    let mut base = raw.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return DEFAULT_EMBEDDING_BASE.to_string();
    }
    if base.ends_with("/embeddings") {
        base = base.trim_end_matches("/embeddings").to_string();
    }
    if !base.ends_with("/v1") && !base.ends_with("/v4") {
        base.push_str("/v1");
    }
    base
}

fn normalize_model(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return DEFAULT_EMBEDDING_MODEL.to_string();
    }
    for prefix in [
        "openai/", "openai:", "google/", "gemini/", "voyage/", "mistral/",
    ] {
        if let Some(stripped) = trimmed.strip_prefix(prefix) {
            let value = stripped.trim();
            if !value.is_empty() {
                return value.to_string();
            }
        }
    }
    trimmed.to_string()
}

fn known_max_input_bytes(provider: &str, model: &str) -> usize {
    let key = format!(
        "{}:{}",
        provider.to_ascii_lowercase(),
        model.to_ascii_lowercase()
    );
    match key.as_str() {
        "openai:text-embedding-3-small"
        | "openai:text-embedding-3-large"
        | "openai:text-embedding-ada-002"
        | "gemini:gemini-embedding-2-preview"
        | "voyage:voyage-3"
        | "voyage:voyage-code-3" => 8192,
        "gemini:text-embedding-004" | "gemini:gemini-embedding-001" => 2048,
        "local:default" => 2048,
        _ => DEFAULT_MAX_INPUT_BYTES,
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    status.as_u16() == 429 || status.is_server_error()
}

fn retry_delay(attempt: usize) -> Duration {
    let factor = 2_u64.saturating_pow((attempt.saturating_sub(1)).min(4) as u32);
    Duration::from_millis((200_u64.saturating_mul(factor)).min(3_000))
}

fn truncate_for_log(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect::<String>() + "..."
}

fn split_text_to_utf8_byte_limit(text: &str, max_utf8_bytes: usize) -> Vec<String> {
    if text.is_empty() || max_utf8_bytes == 0 || text.len() <= max_utf8_bytes {
        return vec![text.to_string()];
    }

    let mut parts = Vec::new();
    let mut cursor = 0usize;
    while cursor < text.len() {
        let mut end = (cursor + max_utf8_bytes).min(text.len());
        while end > cursor && !text.is_char_boundary(end) {
            end -= 1;
        }
        if end == cursor {
            if let Some(next) = text[cursor..].chars().next() {
                end = cursor + next.len_utf8();
            } else {
                break;
            }
        }
        parts.push(text[cursor..end].to_string());
        cursor = end;
    }

    if parts.is_empty() {
        return vec![text.to_string()];
    }
    parts
}

fn parse_embedding_response(payload: serde_json::Value) -> AppResult<Vec<f32>> {
    let embedding = payload
        .get("data")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("embedding"))
        .and_then(|value| value.as_array())
        .or_else(|| {
            payload
                .get("embedding")
                .and_then(|value| value.get("values"))
                .and_then(|value| value.as_array())
        })
        .ok_or_else(|| AppError::new("embedding response missing vector data"))?;

    let mut vector = Vec::with_capacity(embedding.len());
    for value in embedding {
        let number = value
            .as_f64()
            .ok_or_else(|| AppError::new("embedding response contains non-numeric value"))?;
        vector.push(number as f32);
    }
    Ok(sanitize_and_normalize_embedding(vector))
}

fn sanitize_and_normalize_embedding(vector: Vec<f32>) -> Vec<f32> {
    let mut sanitized = vector
        .into_iter()
        .map(|value| if value.is_finite() { value } else { 0.0 })
        .collect::<Vec<_>>();
    let magnitude = sanitized
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if magnitude < 1e-10 {
        return sanitized;
    }
    for value in &mut sanitized {
        *value /= magnitude;
    }
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_preserves_original_text() {
        let content = "甲".repeat(2_000);
        let parts = split_text_to_utf8_byte_limit(&content, 512);
        assert!(parts.len() > 1);
        let merged = parts.join("");
        assert_eq!(merged, content);
        assert!(parts.iter().all(|part| part.len() <= 512));
    }

    #[test]
    fn sanitize_normalizes_vector() {
        let vector = sanitize_and_normalize_embedding(vec![3.0, 4.0]);
        assert!((vector[0] - 0.6).abs() < 1e-6);
        assert!((vector[1] - 0.8).abs() < 1e-6);
    }
}
