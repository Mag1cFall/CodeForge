use std::collections::BTreeMap;
use std::time::Instant;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};

use crate::error::{AppError, AppResult};

use super::model::{ChatRequest, ChatResponse, ProviderRecord, StreamChunk, TokenUsage, ToolCall};
use super::provider::LlmProvider;
use super::streaming::consume_sse_events;
use super::telemetry::log_event;

const RETRY_DELAYS_MS: [u64; 5] = [0, 400, 1200, 3000, 6000];

#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    record: ProviderRecord,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(record: ProviderRecord) -> Self {
        Self {
            record,
            client: reqwest::Client::new(),
        }
    }

    fn endpoint(&self) -> String {
        if self.record.endpoint.ends_with("/messages") {
            self.record.endpoint.clone()
        } else {
            format!("{}/messages", self.record.endpoint.trim_end_matches('/'))
        }
    }

    fn build_headers(&self) -> AppResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static("2023-06-01"),
        );

        if let Some(api_key) = self
            .record
            .api_key
            .as_ref()
            .filter(|key| !key.trim().is_empty())
        {
            headers.insert(
                HeaderName::from_static("x-api-key"),
                HeaderValue::from_str(api_key.trim())
                    .map_err(|error| AppError::new(error.to_string()))?,
            );
        }

        for (key, value) in &self.record.extra.headers {
            headers.insert(
                HeaderName::from_bytes(key.as_bytes())
                    .map_err(|error| AppError::new(error.to_string()))?,
                HeaderValue::from_str(value).map_err(|error| AppError::new(error.to_string()))?,
            );
        }

        Ok(headers)
    }

    fn build_messages(&self, request: &ChatRequest) -> Vec<serde_json::Value> {
        let mut messages = Vec::new();

        for message in &request.messages {
            let role = message.role.trim().to_ascii_lowercase();
            let content = message.content.trim();

            if role == "assistant" {
                if content.is_empty() {
                    continue;
                }
                messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": [{
                        "type": "text",
                        "text": content,
                    }],
                }));
                continue;
            }

            if role == "tool" {
                if let Some((tool_use_id, tool_result_content)) = parse_tool_result_payload(content)
                {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": tool_result_content,
                        }],
                    }));
                } else if !content.is_empty() {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": content,
                    }));
                }
                continue;
            }

            if content.is_empty() {
                continue;
            }

            messages.push(serde_json::json!({
                "role": "user",
                "content": content,
            }));
        }

        messages
    }

    fn build_payload(&self, request: &ChatRequest, stream: bool) -> serde_json::Value {
        let messages = self.build_messages(request);

        let mut payload = serde_json::json!({
            "model": request.model.clone().unwrap_or_else(|| self.record.model.clone()),
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(1024),
            "stream": stream,
        });

        if let Some(system_prompt) = request
            .system_prompt
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            payload["system"] = serde_json::json!(system_prompt);
        }

        if let Some(temperature) = request.temperature {
            payload["temperature"] = serde_json::json!(temperature);
        }

        if !request.tools.is_empty() {
            payload["tools"] = serde_json::json!(request
                .tools
                .iter()
                .map(|tool| {
                    serde_json::json!({
                        "name": tool.name,
                        "description": tool.description,
                        "input_schema": tool.parameters,
                    })
                })
                .collect::<Vec<_>>());
            payload["tool_choice"] = serde_json::json!({ "type": "auto" });
        }

        payload
    }

    async fn post_with_retry(&self, payload: &serde_json::Value) -> AppResult<reqwest::Response> {
        let mut last_error = None;
        let endpoint = self.endpoint();
        let stream_mode = payload
            .get("stream")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        for (attempt, delay_ms) in RETRY_DELAYS_MS.iter().enumerate() {
            let attempt_index = attempt + 1;
            if *delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
            }

            let started_at = Instant::now();
            log_event(
                "anthropic",
                "request_attempt",
                serde_json::json!({
                    "attempt": attempt_index,
                    "maxAttempts": RETRY_DELAYS_MS.len(),
                    "endpoint": endpoint.as_str(),
                    "stream": stream_mode,
                    "model": payload.get("model").and_then(|value| value.as_str()).unwrap_or(self.record.model.as_str()),
                    "messages": payload.get("messages").and_then(|value| value.as_array()).map(Vec::len).unwrap_or(0),
                }),
            );

            let response = self
                .client
                .post(endpoint.as_str())
                .headers(self.build_headers()?)
                .json(payload)
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    log_event(
                        "anthropic",
                        "request_network_error",
                        serde_json::json!({
                            "attempt": attempt_index,
                            "elapsedMs": started_at.elapsed().as_millis() as u64,
                            "error": error.to_string(),
                        }),
                    );
                    last_error = Some(AppError::new(format!("Anthropic 请求网络错误: {}", error)));
                    continue;
                }
            };

            let status = response.status();
            if status.is_success() {
                log_event(
                    "anthropic",
                    "request_success",
                    serde_json::json!({
                        "attempt": attempt_index,
                        "elapsedMs": started_at.elapsed().as_millis() as u64,
                        "status": status.as_u16(),
                        "stream": stream_mode,
                    }),
                );
                return Ok(response);
            }

            let status_code = status.as_u16();
            let should_retry = status.is_server_error() || status_code == 429;
            let body_preview = truncate_for_error(&response.text().await.unwrap_or_default());
            let error_message = format!(
                "Anthropic 请求失败: status={} body={}",
                status_code, body_preview
            );
            log_event(
                "anthropic",
                "request_http_error",
                serde_json::json!({
                    "attempt": attempt_index,
                    "elapsedMs": started_at.elapsed().as_millis() as u64,
                    "status": status_code,
                    "retry": should_retry,
                    "bodyPreview": body_preview,
                }),
            );
            last_error = Some(AppError::new(error_message));
            if should_retry {
                continue;
            }
            return Err(last_error.expect("request error should exist"));
        }

        Err(last_error.unwrap_or_else(|| AppError::new("Anthropic request failed")))
    }

    fn parse_response(&self, value: serde_json::Value) -> AppResult<ChatResponse> {
        let content_items = value
            .get("content")
            .and_then(|content| content.as_array())
            .cloned()
            .unwrap_or_default();

        let text = content_items
            .iter()
            .filter_map(|item| item.get("text").and_then(|value| value.as_str()))
            .collect::<String>();

        let tool_calls = content_items
            .iter()
            .filter(|item| item.get("type").and_then(|value| value.as_str()) == Some("tool_use"))
            .map(|item| ToolCall {
                id: item
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                name: item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                arguments: item
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({})),
            })
            .collect::<Vec<_>>();

        if text.trim().is_empty() && tool_calls.is_empty() {
            return Err(AppError::new("Anthropic 响应缺少可用输出"));
        }

        let usage = parse_usage(value.get("usage"));
        let response = ChatResponse {
            id: value
                .get("id")
                .and_then(|item| item.as_str())
                .map(ToOwned::to_owned),
            model: value
                .get("model")
                .and_then(|item| item.as_str())
                .unwrap_or(self.record.model.as_str())
                .to_string(),
            content: text,
            tool_calls,
            finish_reason: value
                .get("stop_reason")
                .and_then(|item| item.as_str())
                .map(ToOwned::to_owned),
            usage,
        };

        log_event(
            "anthropic",
            "response_parsed",
            serde_json::json!({
                "responseId": response.id.as_deref(),
                "model": response.model.as_str(),
                "finishReason": response.finish_reason.as_deref(),
                "contentChars": response.content.chars().count(),
                "toolCalls": response.tool_calls.len(),
                "inputTokens": response.usage.input_tokens,
                "outputTokens": response.usage.output_tokens,
            }),
        );

        Ok(response)
    }
}

#[derive(Default)]
struct PartialAnthropicToolCall {
    id: String,
    name: String,
    input_json: String,
}

fn parse_partial_input(arguments: &str) -> serde_json::Value {
    let trimmed = arguments.trim();
    if trimmed.is_empty() {
        return serde_json::json!({});
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| serde_json::json!({ "raw": trimmed }))
}

fn parse_tool_result_payload(content: &str) -> Option<(String, serde_json::Value)> {
    let parsed = serde_json::from_str::<serde_json::Value>(content).ok()?;
    let tool_use_id = parsed
        .get("tool_use_id")
        .and_then(|value| value.as_str())
        .or_else(|| parsed.get("toolUseId").and_then(|value| value.as_str()))
        .or_else(|| parsed.get("tool_call_id").and_then(|value| value.as_str()))
        .or_else(|| parsed.get("toolCallId").and_then(|value| value.as_str()))
        .or_else(|| parsed.get("id").and_then(|value| value.as_str()))?
        .to_string();

    let result = parsed
        .get("content")
        .cloned()
        .or_else(|| parsed.get("result").cloned())
        .unwrap_or_else(|| serde_json::json!(content));
    Some((tool_use_id, result))
}

fn parse_usage(value: Option<&serde_json::Value>) -> TokenUsage {
    let input_tokens = value
        .and_then(|usage| usage.get("input_tokens"))
        .and_then(|item| item.as_u64())
        .unwrap_or(0);
    let output_tokens = value
        .and_then(|usage| usage.get("output_tokens"))
        .and_then(|item| item.as_u64())
        .unwrap_or(0);
    TokenUsage {
        input_tokens: input_tokens as usize,
        output_tokens: output_tokens as usize,
    }
}

fn truncate_for_error(value: &str) -> String {
    const LIMIT: usize = 512;
    if value.len() <= LIMIT {
        return value.to_string();
    }
    format!("{}...(truncated)", &value[..LIMIT])
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, request: ChatRequest) -> AppResult<ChatResponse> {
        let payload = self.build_payload(&request, false);
        log_event(
            "anthropic",
            "chat_started",
            serde_json::json!({
                "providerId": self.record.id.as_str(),
                "endpoint": self.endpoint(),
                "model": payload.get("model").and_then(|value| value.as_str()).unwrap_or(self.record.model.as_str()),
                "messages": payload.get("messages").and_then(|value| value.as_array()).map(Vec::len).unwrap_or(0),
                "tools": payload.get("tools").and_then(|value| value.as_array()).map(Vec::len).unwrap_or(0),
            }),
        );

        let response = self.post_with_retry(&payload).await?;
        let value = response.json().await?;
        self.parse_response(value)
    }

    async fn chat_stream(&self, request: ChatRequest) -> AppResult<Vec<StreamChunk>> {
        let payload = self.build_payload(&request, true);
        log_event(
            "anthropic",
            "chat_stream_started",
            serde_json::json!({
                "providerId": self.record.id.as_str(),
                "endpoint": self.endpoint(),
                "model": payload.get("model").and_then(|value| value.as_str()).unwrap_or(self.record.model.as_str()),
                "messages": payload.get("messages").and_then(|value| value.as_array()).map(Vec::len).unwrap_or(0),
                "tools": payload.get("tools").and_then(|value| value.as_array()).map(Vec::len).unwrap_or(0),
            }),
        );

        let response = self.post_with_retry(&payload).await?;

        let (events, done_seen) = consume_sse_events(response).await?;
        let mut partial_tool_calls: BTreeMap<usize, PartialAnthropicToolCall> = BTreeMap::new();
        let mut chunks = Vec::new();
        let mut last_finish_reason = None;
        let mut usage_input_tokens = 0_usize;
        let mut usage_output_tokens = 0_usize;

        for event in events {
            let event_type = event
                .event
                .clone()
                .or_else(|| {
                    event
                        .payload
                        .get("type")
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned)
                })
                .unwrap_or_default();
            let mut delta = String::new();
            let mut tool_calls = Vec::new();
            let mut finish_reason = None;

            match event_type.as_str() {
                "message_start" => {
                    if let Some(usage) = event
                        .payload
                        .get("message")
                        .and_then(|value| value.get("usage"))
                    {
                        usage_input_tokens = usage
                            .get("input_tokens")
                            .and_then(|value| value.as_u64())
                            .unwrap_or(0) as usize;
                        usage_output_tokens = usage
                            .get("output_tokens")
                            .and_then(|value| value.as_u64())
                            .unwrap_or(0) as usize;
                    }
                }
                "content_block_start" => {
                    if event
                        .payload
                        .get("content_block")
                        .and_then(|value| value.get("type"))
                        .and_then(|value| value.as_str())
                        == Some("tool_use")
                    {
                        let index = event
                            .payload
                            .get("index")
                            .and_then(|value| value.as_u64())
                            .unwrap_or(0) as usize;
                        let block = event
                            .payload
                            .get("content_block")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!({}));
                        let entry = partial_tool_calls.entry(index).or_default();
                        entry.id = block
                            .get("id")
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string();
                        entry.name = block
                            .get("name")
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string();
                        if let Some(input) = block.get("input") {
                            entry.input_json = input.to_string();
                        }
                        tool_calls.push(ToolCall {
                            id: entry.id.clone(),
                            name: entry.name.clone(),
                            arguments: parse_partial_input(&entry.input_json),
                        });
                    }
                }
                "content_block_delta" => {
                    let delta_payload = event
                        .payload
                        .get("delta")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({}));

                    if delta_payload.get("type").and_then(|value| value.as_str())
                        == Some("text_delta")
                    {
                        delta = delta_payload
                            .get("text")
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string();
                    }

                    if let Some(partial_json) = delta_payload
                        .get("partial_json")
                        .and_then(|value| value.as_str())
                    {
                        let index = event
                            .payload
                            .get("index")
                            .and_then(|value| value.as_u64())
                            .unwrap_or(0) as usize;
                        let entry = partial_tool_calls.entry(index).or_default();
                        entry.input_json.push_str(partial_json);
                        tool_calls.push(ToolCall {
                            id: entry.id.clone(),
                            name: entry.name.clone(),
                            arguments: parse_partial_input(&entry.input_json),
                        });
                    }
                }
                "content_block_stop" => {
                    let index = event
                        .payload
                        .get("index")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0) as usize;
                    if let Some(entry) = partial_tool_calls.get(&index) {
                        tool_calls.push(ToolCall {
                            id: entry.id.clone(),
                            name: entry.name.clone(),
                            arguments: parse_partial_input(&entry.input_json),
                        });
                    }
                }
                "message_delta" => {
                    finish_reason = event
                        .payload
                        .get("delta")
                        .and_then(|value| value.get("stop_reason"))
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned);

                    if let Some(usage) = event.payload.get("usage") {
                        usage_input_tokens = usage
                            .get("input_tokens")
                            .and_then(|value| value.as_u64())
                            .unwrap_or(usage_input_tokens as u64)
                            as usize;
                        usage_output_tokens = usage
                            .get("output_tokens")
                            .and_then(|value| value.as_u64())
                            .unwrap_or(usage_output_tokens as u64)
                            as usize;
                    }
                }
                "message_stop" => {
                    finish_reason = Some("stop".into());
                }
                _ => {}
            }

            if finish_reason.is_some() {
                last_finish_reason = finish_reason.clone();
            }

            let done = finish_reason.is_some();
            if !delta.is_empty() || !tool_calls.is_empty() || done {
                chunks.push(StreamChunk {
                    delta,
                    done,
                    tool_calls,
                    finish_reason,
                });
            }
        }

        if chunks.is_empty() || (!done_seen && !chunks.last().is_some_and(|chunk| chunk.done)) {
            chunks.push(StreamChunk {
                delta: String::new(),
                done: true,
                tool_calls: vec![],
                finish_reason: last_finish_reason.or_else(|| Some("stop".into())),
            });
        }

        log_event(
            "anthropic",
            "chat_stream_completed",
            serde_json::json!({
                "chunks": chunks.len(),
                "doneSeen": done_seen,
                "lastFinishReason": chunks.last().and_then(|chunk| chunk.finish_reason.as_ref()),
                "toolCallChunks": chunks.iter().filter(|chunk| !chunk.tool_calls.is_empty()).count(),
                "inputTokens": usage_input_tokens,
                "outputTokens": usage_output_tokens,
            }),
        );

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::llm::model::{ChatMessage, ProviderExtra, ProviderType};

    #[test]
    fn builds_anthropic_payload() {
        let provider = AnthropicProvider::new(ProviderRecord {
            id: "provider-1".into(),
            name: "Anthropic".into(),
            provider_type: ProviderType::Anthropic,
            endpoint: "https://api.anthropic.com/v1".into(),
            api_key: Some("secret".into()),
            model: "claude-sonnet-4-5".into(),
            extra: ProviderExtra {
                models: vec!["claude-sonnet-4-5".into()],
                headers: BTreeMap::new(),
                context_windows: BTreeMap::new(),
            },
            enabled: true,
            is_default: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        });

        let payload = provider.build_payload(
            &ChatRequest {
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: "hello".into(),
                }],
                system_prompt: Some("You are helpful".into()),
                model: None,
                max_tokens: Some(256),
                temperature: Some(0.1),
                tools: vec![],
            },
            false,
        );

        assert_eq!(payload["model"], "claude-sonnet-4-5");
        assert_eq!(payload["system"], "You are helpful");
        assert_eq!(payload["messages"].as_array().map(Vec::len), Some(1));
    }
}
