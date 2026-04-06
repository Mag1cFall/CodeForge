use std::collections::BTreeMap;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};

use crate::error::{AppError, AppResult};

use super::model::{ChatRequest, ChatResponse, ProviderRecord, StreamChunk, TokenUsage, ToolCall};
use super::provider::LlmProvider;
use super::streaming::consume_sse_events;

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    record: ProviderRecord,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(record: ProviderRecord) -> Self {
        Self {
            record,
            client: reqwest::Client::new(),
        }
    }

    fn build_headers(&self) -> AppResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(api_key) = self.record.api_key.as_ref().filter(|key| !key.trim().is_empty()) {
            let token = format!("Bearer {}", api_key.trim());
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&token).map_err(|error| AppError::new(error.to_string()))?,
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

    fn endpoint(&self) -> String {
        if self.record.endpoint.ends_with("/chat/completions") {
            self.record.endpoint.clone()
        } else {
            format!(
                "{}/chat/completions",
                self.record.endpoint.trim_end_matches('/')
            )
        }
    }

    fn build_payload(&self, request: &ChatRequest, stream: bool) -> serde_json::Value {
        let mut messages = Vec::new();
        if let Some(system_prompt) = request.system_prompt.as_ref().filter(|value| !value.trim().is_empty()) {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system_prompt,
            }));
        }

        messages.extend(request.messages.iter().map(|message| {
            serde_json::json!({
                "role": message.role,
                "content": message.content,
            })
        }));

        let tools = if request.tools.is_empty() {
            None
        } else {
            Some(
                request
                    .tools
                    .iter()
                    .map(|tool| {
                        serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": tool.name,
                                "description": tool.description,
                                "parameters": tool.parameters,
                            }
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let mut payload = serde_json::json!({
            "model": request.model.clone().unwrap_or_else(|| self.record.model.clone()),
            "messages": messages,
            "stream": stream,
        });

        if let Some(max_tokens) = request.max_tokens {
            payload["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(temperature) = request.temperature {
            payload["temperature"] = serde_json::json!(temperature);
        }
        if let Some(tools) = tools {
            payload["tools"] = serde_json::json!(tools);
        }

        payload
    }

    fn parse_response(&self, value: serde_json::Value) -> AppResult<ChatResponse> {
        let choice = value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .ok_or_else(|| AppError::new("OpenAI compatible 响应缺少 choices[0]"))?;

        let message = choice
            .get("message")
            .ok_or_else(|| AppError::new("OpenAI compatible 响应缺少 message"))?;

        let tool_calls = message
            .get("tool_calls")
            .and_then(|calls| calls.as_array())
            .map(|calls| {
                calls
                    .iter()
                    .map(|call| ToolCall {
                        id: call.get("id").and_then(|value| value.as_str()).unwrap_or_default().to_string(),
                        name: call
                            .get("function")
                            .and_then(|value| value.get("name"))
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        arguments: call
                            .get("function")
                            .and_then(|value| value.get("arguments"))
                            .map(|value| {
                                if let Some(text) = value.as_str() {
                                    serde_json::from_str(text).unwrap_or_else(|_| serde_json::json!({ "raw": text }))
                                } else {
                                    value.clone()
                                }
                            })
                            .unwrap_or_else(|| serde_json::json!({})),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let usage = value.get("usage").cloned().unwrap_or_default();
        Ok(ChatResponse {
            id: value.get("id").and_then(|item| item.as_str()).map(ToOwned::to_owned),
            model: value
                .get("model")
                .and_then(|item| item.as_str())
                .unwrap_or(self.record.model.as_str())
                .to_string(),
            content: message
                .get("content")
                .and_then(|item| item.as_str())
                .unwrap_or_default()
                .to_string(),
            tool_calls,
            finish_reason: choice
                .get("finish_reason")
                .and_then(|item| item.as_str())
                .map(ToOwned::to_owned),
            usage: TokenUsage {
                input_tokens: usage.get("prompt_tokens").and_then(|item| item.as_u64()).unwrap_or(0) as usize,
                output_tokens: usage.get("completion_tokens").and_then(|item| item.as_u64()).unwrap_or(0) as usize,
            },
        })
    }
}

#[derive(Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

fn parse_tool_arguments(arguments: &str) -> serde_json::Value {
    serde_json::from_str(arguments).unwrap_or_else(|_| serde_json::json!({ "raw": arguments }))
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn chat(&self, request: ChatRequest) -> AppResult<ChatResponse> {
        let response = self
            .client
            .post(self.endpoint())
            .headers(self.build_headers()?)
            .json(&self.build_payload(&request, false))
            .send()
            .await?
            .error_for_status()?;

        self.parse_response(response.json().await?)
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
        let mut partial_tool_calls: BTreeMap<usize, PartialToolCall> = BTreeMap::new();
        let mut chunks = Vec::new();

        for event in events {
            let Some(choice) = event.payload.get("choices").and_then(|choices| choices.get(0)) else {
                continue;
            };
            let delta_payload = choice.get("delta").cloned().unwrap_or_else(|| serde_json::json!({}));
            let delta_text = delta_payload
                .get("content")
                .and_then(|content| content.as_str())
                .unwrap_or_default()
                .to_string();
            let finish_reason = choice
                .get("finish_reason")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);

            let mut tool_calls = Vec::new();
            if let Some(items) = delta_payload.get("tool_calls").and_then(|value| value.as_array()) {
                for item in items {
                    let index = item.get("index").and_then(|value| value.as_u64()).unwrap_or(0) as usize;
                    let entry = partial_tool_calls.entry(index).or_default();
                    if let Some(id) = item.get("id").and_then(|value| value.as_str()) {
                        entry.id = id.to_string();
                    }
                    if let Some(name) = item
                        .get("function")
                        .and_then(|value| value.get("name"))
                        .and_then(|value| value.as_str())
                    {
                        entry.name = name.to_string();
                    }
                    if let Some(arguments) = item
                        .get("function")
                        .and_then(|value| value.get("arguments"))
                        .and_then(|value| value.as_str())
                    {
                        entry.arguments.push_str(arguments);
                    }

                    tool_calls.push(ToolCall {
                        id: entry.id.clone(),
                        name: entry.name.clone(),
                        arguments: parse_tool_arguments(&entry.arguments),
                    });
                }
            }

            let done = finish_reason.is_some();
            if !delta_text.is_empty() || !tool_calls.is_empty() || done {
                chunks.push(StreamChunk {
                    delta: delta_text,
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
    fn builds_openai_payload_with_tools() {
        let provider = OpenAiCompatibleProvider::new(ProviderRecord {
            id: "provider-1".into(),
            name: "OpenAI".into(),
            provider_type: ProviderType::OpenAiCompatible,
            endpoint: "https://example.com/v1/chat/completions".into(),
            api_key: Some("secret".into()),
            model: "gpt-5.4-mini".into(),
            extra: ProviderExtra {
                models: vec!["gpt-5.4-mini".into()],
                headers: BTreeMap::new(),
                context_windows: BTreeMap::new(),
            },
            enabled: true,
            is_default: true,
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
                tools: vec![crate::llm::model::ToolDefinition {
                    name: "read_file".into(),
                    description: "Read a file".into(),
                    parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" } } }),
                }],
            },
            false,
        );

        assert_eq!(payload["stream"], false);
        assert_eq!(payload["model"], "gpt-5.4-mini");
        assert_eq!(payload["messages"].as_array().map(Vec::len), Some(2));
        assert_eq!(payload["tools"].as_array().map(Vec::len), Some(1));
    }

    #[tokio::test]
    #[ignore]
    async fn live_openai_compatible_chat() {
        let endpoint = std::env::var("CODEFORGE_LIVE_LLM_ENDPOINT").expect("CODEFORGE_LIVE_LLM_ENDPOINT required");
        let api_key = std::env::var("CODEFORGE_LIVE_LLM_API_KEY").expect("CODEFORGE_LIVE_LLM_API_KEY required");
        let model = std::env::var("CODEFORGE_LIVE_LLM_MODEL").expect("CODEFORGE_LIVE_LLM_MODEL required");

        let provider = OpenAiCompatibleProvider::new(ProviderRecord {
            id: "live-provider".into(),
            name: "Live OpenAI Compatible".into(),
            provider_type: ProviderType::OpenAiCompatible,
            endpoint,
            api_key: Some(api_key),
            model: model.clone(),
            extra: ProviderExtra {
                models: vec![model.clone()],
                headers: BTreeMap::new(),
                context_windows: BTreeMap::new(),
            },
            enabled: true,
            is_default: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        });

        let response = provider
            .chat(ChatRequest {
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: "hello from CodeForge live test".into(),
                }],
                system_prompt: Some("Reply in one short sentence.".into()),
                model: Some(model),
                max_tokens: Some(64),
                temperature: Some(0.1),
                tools: vec![],
            })
            .await
            .expect("live chat should succeed");

        assert!(!response.content.trim().is_empty());
        println!("LIVE_OPENAI_RESPONSE={}", response.content.trim());
    }
}
