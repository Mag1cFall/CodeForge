use std::collections::BTreeMap;
use std::time::Instant;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};

use crate::error::{AppError, AppResult};

use super::model::{ChatRequest, ChatResponse, ProviderRecord, StreamChunk, TokenUsage, ToolCall};
use super::provider::LlmProvider;
use super::streaming::consume_sse_events;
use super::telemetry::log_event;

const RETRY_DELAYS_MS: [u64; 5] = [0, 500, 1500, 4000, 8000];
const EMPTY_OUTPUT_FALLBACK: &str = "模型返回了空响应，请检查 Provider 与模型配置后重试。";

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

        if let Some(api_key) = self
            .record
            .api_key
            .as_ref()
            .filter(|key| !key.trim().is_empty())
        {
            let token = format!("Bearer {}", api_key.trim());
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&token).map_err(|error| AppError::new(error.to_string()))?,
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
        if let Some(system_prompt) = request
            .system_prompt
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system_prompt,
            }));
        }

        for message in &request.messages {
            if message.role == "assistant" && message.content.trim().is_empty() {
                continue;
            }
            messages.push(serde_json::json!({
                "role": message.role,
                "content": message.content,
            }));
        }

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
        if let Some(top_p) = request.top_p {
            payload["top_p"] = serde_json::json!(top_p);
        }
        if let Some(tools) = tools {
            payload["tools"] = serde_json::json!(tools);
            payload["tool_choice"] = serde_json::json!("auto");
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
                "openai_compatible",
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
                        "openai_compatible",
                        "request_network_error",
                        serde_json::json!({
                            "attempt": attempt_index,
                            "elapsedMs": started_at.elapsed().as_millis() as u64,
                            "error": error.to_string(),
                        }),
                    );
                    last_error = Some(AppError::new(format!(
                        "OpenAI Compatible 请求网络错误: {}",
                        error
                    )));
                    continue;
                }
            };

            let status = response.status();
            if status.is_success() {
                log_event(
                    "openai_compatible",
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
                "OpenAI Compatible 请求失败: status={} body={}",
                status_code, body_preview
            );
            log_event(
                "openai_compatible",
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

        Err(last_error.unwrap_or_else(|| AppError::new("LLM request failed")))
    }

    fn parse_response(&self, value: serde_json::Value) -> AppResult<ChatResponse> {
        let choice = value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .ok_or_else(|| AppError::new("OpenAI compatible 响应缺少 choices[0]"))?;

        let message = choice
            .get("message")
            .ok_or_else(|| AppError::new("OpenAI compatible 响应缺少 message"))?;

        let content = normalize_openai_content(message.get("content"))
            .or_else(|| normalize_openai_content(message.get("refusal")))
            .or_else(|| normalize_openai_content(message.get("reasoning_content")))
            .or_else(|| normalize_openai_content(message.get("reasoningContent")))
            .or_else(|| normalize_openai_content(choice.get("text")))
            .or_else(|| normalize_openai_content(value.get("output_text")))
            .unwrap_or_default();

        let mut tool_calls = parse_tool_calls(message.get("tool_calls"));
        if tool_calls.is_empty() {
            if let Some(call) = parse_legacy_function_call(message) {
                tool_calls.push(call);
            }
        }
        let content = if content.trim().is_empty() && tool_calls.is_empty() {
            let recovered = recover_text_from_choice(choice)
                .or_else(|| recover_text_from_response(&value));
            log_event(
                "openai_compatible",
                "response_empty_output",
                serde_json::json!({
                    "finishReason": choice.get("finish_reason"),
                    "messageKeys": message
                        .as_object()
                        .map(|item| item.keys().cloned().collect::<Vec<_>>())
                        .unwrap_or_default(),
                    "choiceKeys": choice
                        .as_object()
                        .map(|item| item.keys().cloned().collect::<Vec<_>>())
                        .unwrap_or_default(),
                    "payloadPreview": truncate_for_error(&value.to_string()),
                }),
            );
            recovered.unwrap_or_else(|| EMPTY_OUTPUT_FALLBACK.to_string())
        } else {
            content
        };

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
            content,
            tool_calls,
            finish_reason: choice
                .get("finish_reason")
                .and_then(|item| item.as_str())
                .map(ToOwned::to_owned),
            usage,
        };

        log_event(
            "openai_compatible",
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
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

fn parse_tool_arguments(arguments: &str) -> serde_json::Value {
    let trimmed = arguments.trim();
    if trimmed.is_empty() {
        return serde_json::json!({});
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| serde_json::json!({ "raw": trimmed }))
}

fn truncate_for_error(value: &str) -> String {
    const LIMIT: usize = 512;
    if value.len() <= LIMIT {
        return value.to_string();
    }
    format!("{}...(truncated)", &value[..LIMIT])
}

fn normalize_openai_content(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    if let Some(items) = value.as_array() {
        let text = items
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .and_then(|text| text.as_str())
                    .or_else(|| {
                        item.get("text")
                            .and_then(|text| text.get("value"))
                            .and_then(|text| text.as_str())
                    })
                    .or_else(|| item.get("output_text").and_then(|text| text.as_str()))
                    .or_else(|| {
                        item.get("content")
                            .and_then(|content| content.get("text"))
                            .and_then(|text| text.as_str())
                    })
                    .or_else(|| item.get("content").and_then(|content| content.as_str()))
            })
            .collect::<String>();
        return Some(text);
    }

    if let Some(obj) = value.as_object() {
        if let Some(text) = obj.get("text").and_then(|item| item.as_str()) {
            return Some(text.to_string());
        }
        if let Some(text) = obj
            .get("text")
            .and_then(|item| item.get("value"))
            .and_then(|item| item.as_str())
        {
            return Some(text.to_string());
        }
        if let Some(text) = obj.get("output_text").and_then(|item| item.as_str()) {
            return Some(text.to_string());
        }
    }

    None
}

fn recover_text_from_choice(choice: &serde_json::Value) -> Option<String> {
    normalize_openai_content(choice.get("delta").and_then(|delta| delta.get("content")))
        .or_else(|| normalize_openai_content(choice.get("delta").and_then(|delta| delta.get("text"))))
        .or_else(|| normalize_openai_content(choice.get("message").and_then(|msg| msg.get("reasoning"))))
}

fn recover_text_from_response(value: &serde_json::Value) -> Option<String> {
    normalize_openai_content(value.get("output"))
        .or_else(|| normalize_openai_content(value.get("response")))
        .or_else(|| normalize_openai_content(value.get("data")))
}

fn should_try_stream_recovery(response: &ChatResponse) -> bool {
    response.content == EMPTY_OUTPUT_FALLBACK && response.tool_calls.is_empty()
}

fn collect_tool_calls_from_stream(chunks: &[StreamChunk]) -> Vec<ToolCall> {
    let mut calls = BTreeMap::<String, ToolCall>::new();
    for chunk in chunks {
        for call in &chunk.tool_calls {
            if call.id.trim().is_empty() {
                continue;
            }
            calls.insert(call.id.clone(), call.clone());
        }
    }
    calls.into_values().collect()
}

fn collect_text_from_stream(chunks: &[StreamChunk]) -> String {
    chunks
        .iter()
        .map(|chunk| chunk.delta.as_str())
        .collect::<String>()
}

fn parse_tool_calls(value: Option<&serde_json::Value>) -> Vec<ToolCall> {
    let Some(items) = value.and_then(|calls| calls.as_array()) else {
        return Vec::new();
    };

    items
        .iter()
        .enumerate()
        .map(|(index, call)| {
            let call_value = if let Some(raw) = call.as_str() {
                serde_json::from_str::<serde_json::Value>(raw)
                    .unwrap_or_else(|_| serde_json::json!({}))
            } else {
                call.clone()
            };

            let id = call_value
                .get("id")
                .and_then(|item| item.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| format!("tool_call_{index}"));
            let name = call_value
                .get("function")
                .and_then(|item| item.get("name"))
                .and_then(|item| item.as_str())
                .unwrap_or_default()
                .to_string();
            let arguments = call_value
                .get("function")
                .and_then(|item| item.get("arguments"))
                .map(|item| {
                    if let Some(raw) = item.as_str() {
                        parse_tool_arguments(raw)
                    } else {
                        item.clone()
                    }
                })
                .unwrap_or_else(|| serde_json::json!({}));

            ToolCall {
                id,
                name,
                arguments,
            }
        })
        .collect()
}

fn parse_legacy_function_call(message: &serde_json::Value) -> Option<ToolCall> {
    let call = message
        .get("function_call")
        .or_else(|| message.get("functionCall"))?;

    let name = call
        .get("name")
        .and_then(|item| item.as_str())
        .unwrap_or_default()
        .to_string();
    if name.trim().is_empty() {
        return None;
    }

    let arguments = call
        .get("arguments")
        .map(|item| {
            if let Some(raw) = item.as_str() {
                parse_tool_arguments(raw)
            } else {
                item.clone()
            }
        })
        .unwrap_or_else(|| serde_json::json!({}));

    let id = call
        .get("id")
        .and_then(|item| item.as_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "legacy_function_call".to_string());

    Some(ToolCall {
        id,
        name,
        arguments,
    })
}

fn parse_usage(value: Option<&serde_json::Value>) -> TokenUsage {
    let prompt_tokens = value
        .and_then(|usage| usage.get("prompt_tokens").and_then(|item| item.as_u64()))
        .or_else(|| {
            value.and_then(|usage| usage.get("input_tokens").and_then(|item| item.as_u64()))
        })
        .unwrap_or(0);
    let completion_tokens = value
        .and_then(|usage| {
            usage
                .get("completion_tokens")
                .and_then(|item| item.as_u64())
        })
        .or_else(|| {
            value.and_then(|usage| usage.get("output_tokens").and_then(|item| item.as_u64()))
        })
        .unwrap_or(0);

    TokenUsage {
        input_tokens: prompt_tokens as usize,
        output_tokens: completion_tokens as usize,
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn chat(&self, request: ChatRequest) -> AppResult<ChatResponse> {
        let payload = self.build_payload(&request, false);
        log_event(
            "openai_compatible",
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
        let response_json = response.json().await?;
        let mut parsed = self.parse_response(response_json)?;

        if should_try_stream_recovery(&parsed) {
            log_event(
                "openai_compatible",
                "stream_recovery_started",
                serde_json::json!({
                    "providerId": self.record.id.as_str(),
                    "model": parsed.model.as_str(),
                }),
            );

            let chunks = self.chat_stream(request.clone()).await?;
            let recovered_text = collect_text_from_stream(&chunks);
            let recovered_tool_calls = collect_tool_calls_from_stream(&chunks);

            if !recovered_text.trim().is_empty() || !recovered_tool_calls.is_empty() {
                parsed.content = recovered_text;
                parsed.tool_calls = recovered_tool_calls;
                if parsed.finish_reason.is_none() {
                    parsed.finish_reason = chunks
                        .iter()
                        .rev()
                        .find_map(|chunk| chunk.finish_reason.clone());
                }
                log_event(
                    "openai_compatible",
                    "stream_recovery_success",
                    serde_json::json!({
                        "providerId": self.record.id.as_str(),
                        "chars": parsed.content.chars().count(),
                        "toolCalls": parsed.tool_calls.len(),
                    }),
                );
            } else {
                log_event(
                    "openai_compatible",
                    "stream_recovery_empty",
                    serde_json::json!({
                        "providerId": self.record.id.as_str(),
                        "chunks": chunks.len(),
                    }),
                );
            }
        }

        Ok(parsed)
    }

    async fn chat_stream(&self, request: ChatRequest) -> AppResult<Vec<StreamChunk>> {
        let payload = self.build_payload(&request, true);
        log_event(
            "openai_compatible",
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
        let mut partial_tool_calls: BTreeMap<usize, PartialToolCall> = BTreeMap::new();
        let mut chunks = Vec::new();
        let mut last_finish_reason = None;
        let mut usage_event_count = 0_usize;

        for event in events {
            if event
                .payload
                .get("usage")
                .and_then(|value| value.as_object())
                .is_some()
            {
                usage_event_count += 1;
            }

            let Some(choice) = event
                .payload
                .get("choices")
                .and_then(|choices| choices.get(0))
            else {
                continue;
            };
            let delta_payload = choice
                .get("delta")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let delta_text =
                normalize_openai_content(delta_payload.get("content")).unwrap_or_default();
            let finish_reason = choice
                .get("finish_reason")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            if finish_reason.is_some() {
                last_finish_reason = finish_reason.clone();
            }

            let mut tool_calls = Vec::new();
            if let Some(items) = delta_payload
                .get("tool_calls")
                .and_then(|value| value.as_array())
            {
                for (fallback_index, item) in items.iter().enumerate() {
                    let index = item
                        .get("index")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(fallback_index as u64) as usize;
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
                    {
                        if let Some(arguments) = arguments.as_str() {
                            entry.arguments.push_str(arguments);
                        } else {
                            entry.arguments = arguments.to_string();
                        }
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
                finish_reason: last_finish_reason.or_else(|| Some("stop".into())),
            });
        }

        log_event(
            "openai_compatible",
            "chat_stream_completed",
            serde_json::json!({
                "chunks": chunks.len(),
                "doneSeen": done_seen,
                "usageEvents": usage_event_count,
                "lastFinishReason": chunks.last().and_then(|chunk| chunk.finish_reason.as_ref()),
                "toolCallChunks": chunks.iter().filter(|chunk| !chunk.tool_calls.is_empty()).count(),
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
                top_p: None,
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

    #[test]
    fn parses_legacy_function_call_response() {
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

        let response = provider
            .parse_response(serde_json::json!({
                "id": "resp-1",
                "model": "gpt-5.4-mini",
                "choices": [{
                    "finish_reason": "tool_calls",
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "function_call": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"README.md\"}"
                        }
                    }
                }],
                "usage": {
                    "prompt_tokens": 1,
                    "completion_tokens": 1
                }
            }))
            .expect("legacy function_call should be parsed");

        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "read_file");
    }

    #[tokio::test]
    #[ignore]
    async fn live_openai_compatible_chat() {
        let endpoint = std::env::var("CODEFORGE_LIVE_LLM_ENDPOINT")
            .expect("CODEFORGE_LIVE_LLM_ENDPOINT required");
        let api_key = std::env::var("CODEFORGE_LIVE_LLM_API_KEY")
            .expect("CODEFORGE_LIVE_LLM_API_KEY required");
        let model =
            std::env::var("CODEFORGE_LIVE_LLM_MODEL").expect("CODEFORGE_LIVE_LLM_MODEL required");

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
                top_p: None,
                tools: vec![],
            })
            .await
            .expect("live chat should succeed");

        assert!(!response.content.trim().is_empty());
        println!("LIVE_OPENAI_RESPONSE={}", response.content.trim());
    }
}
