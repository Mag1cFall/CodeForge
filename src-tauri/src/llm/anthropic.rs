use std::collections::BTreeMap;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};

use crate::error::{AppError, AppResult};

use super::model::{ChatRequest, ChatResponse, ProviderRecord, StreamChunk, TokenUsage, ToolCall};
use super::provider::LlmProvider;
use super::streaming::consume_sse_events;

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

        if let Some(api_key) = self.record.api_key.as_ref().filter(|key| !key.trim().is_empty()) {
            headers.insert(
                HeaderName::from_static("x-api-key"),
                HeaderValue::from_str(api_key.trim()).map_err(|error| AppError::new(error.to_string()))?,
            );
        }

        for (key, value) in &self.record.extra.headers {
            headers.insert(
                HeaderName::from_bytes(key.as_bytes()).map_err(|error| AppError::new(error.to_string()))?,
                HeaderValue::from_str(value).map_err(|error| AppError::new(error.to_string()))?,
            );
        }

        Ok(headers)
    }

    fn build_payload(&self, request: &ChatRequest, stream: bool) -> serde_json::Value {
        let messages = request
            .messages
            .iter()
            .map(|message| {
                serde_json::json!({
                    "role": if message.role == "assistant" { "assistant" } else { "user" },
                    "content": message.content,
                })
            })
            .collect::<Vec<_>>();

        let mut payload = serde_json::json!({
            "model": request.model.clone().unwrap_or_else(|| self.record.model.clone()),
            "system": request.system_prompt.clone().unwrap_or_default(),
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(1024),
            "stream": stream,
        });

        if let Some(temperature) = request.temperature {
            payload["temperature"] = serde_json::json!(temperature);
        }

        if !request.tools.is_empty() {
            payload["tools"] = serde_json::json!(request.tools.iter().map(|tool| {
                serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.parameters,
                })
            }).collect::<Vec<_>>());
        }

        payload
    }

    fn parse_response(&self, value: serde_json::Value) -> ChatResponse {
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
                id: item.get("id").and_then(|value| value.as_str()).unwrap_or_default().to_string(),
                name: item.get("name").and_then(|value| value.as_str()).unwrap_or_default().to_string(),
                arguments: item.get("input").cloned().unwrap_or_else(|| serde_json::json!({})),
            })
            .collect::<Vec<_>>();

        let usage = value.get("usage").cloned().unwrap_or_default();
        ChatResponse {
            id: value.get("id").and_then(|item| item.as_str()).map(ToOwned::to_owned),
            model: value
                .get("model")
                .and_then(|item| item.as_str())
                .unwrap_or(self.record.model.as_str())
                .to_string(),
            content: text,
            tool_calls,
            finish_reason: value.get("stop_reason").and_then(|item| item.as_str()).map(ToOwned::to_owned),
            usage: TokenUsage {
                input_tokens: usage.get("input_tokens").and_then(|item| item.as_u64()).unwrap_or(0) as usize,
                output_tokens: usage.get("output_tokens").and_then(|item| item.as_u64()).unwrap_or(0) as usize,
            },
        }
    }
}

#[derive(Default)]
struct PartialAnthropicToolCall {
    id: String,
    name: String,
    input_json: String,
}

fn parse_partial_input(arguments: &str) -> serde_json::Value {
    serde_json::from_str(arguments).unwrap_or_else(|_| serde_json::json!({ "raw": arguments }))
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, request: ChatRequest) -> AppResult<ChatResponse> {
        let response = self
            .client
            .post(self.endpoint())
            .headers(self.build_headers()?)
            .json(&self.build_payload(&request, false))
            .send()
            .await?
            .error_for_status()?;
        Ok(self.parse_response(response.json().await?))
    }

    async fn chat_stream(&self, request: ChatRequest) -> AppResult<Vec<StreamChunk>> {
        let response = self
            .client
            .post(self.endpoint())
            .headers(self.build_headers()?)
            .json(&self.build_payload(&request, true))
            .send()
            .await?
            .error_for_status()?;

        let (events, done_seen) = consume_sse_events(response).await?;
        let mut partial_tool_calls: BTreeMap<usize, PartialAnthropicToolCall> = BTreeMap::new();
        let mut chunks = Vec::new();

        for event in events {
            let event_type = event
                .event
                .clone()
                .or_else(|| event.payload.get("type").and_then(|value| value.as_str()).map(ToOwned::to_owned))
                .unwrap_or_default();
            let mut delta = String::new();
            let mut tool_calls = Vec::new();
            let mut finish_reason = None;

            match event_type.as_str() {
                "content_block_start" => {
                    if event.payload.get("content_block").and_then(|value| value.get("type")).and_then(|value| value.as_str()) == Some("tool_use") {
                        let index = event.payload.get("index").and_then(|value| value.as_u64()).unwrap_or(0) as usize;
                        let block = event.payload.get("content_block").cloned().unwrap_or_else(|| serde_json::json!({}));
                        let entry = partial_tool_calls.entry(index).or_default();
                        entry.id = block.get("id").and_then(|value| value.as_str()).unwrap_or_default().to_string();
                        entry.name = block.get("name").and_then(|value| value.as_str()).unwrap_or_default().to_string();
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
                    delta = event
                        .payload
                        .get("delta")
                        .and_then(|value| value.get("text"))
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string();

                    if let Some(partial_json) = event
                        .payload
                        .get("delta")
                        .and_then(|value| value.get("partial_json"))
                        .and_then(|value| value.as_str())
                    {
                        let index = event.payload.get("index").and_then(|value| value.as_u64()).unwrap_or(0) as usize;
                        let entry = partial_tool_calls.entry(index).or_default();
                        entry.input_json.push_str(partial_json);
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
                }
                "message_stop" => {
                    finish_reason = Some("stop".into());
                }
                _ => {}
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
                finish_reason: Some("stop".into()),
            });
        }
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
