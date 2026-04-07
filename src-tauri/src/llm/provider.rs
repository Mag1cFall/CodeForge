use async_trait::async_trait;

use crate::error::{AppError, AppResult};

use super::anthropic::AnthropicProvider;
use super::model::{ChatRequest, ChatResponse, ProviderRecord, ProviderType, StreamChunk};
use super::openai_compatible::OpenAiCompatibleProvider;
use super::telemetry::log_event;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> AppResult<ChatResponse>;
    async fn chat_stream(&self, request: ChatRequest) -> AppResult<Vec<StreamChunk>>;
}

pub fn build_provider(record: ProviderRecord) -> AppResult<Box<dyn LlmProvider>> {
    if !record.enabled {
        log_event(
            "provider",
            "build_rejected_disabled",
            serde_json::json!({
                "providerId": record.id.as_str(),
                "providerType": record.provider_type.as_str(),
            }),
        );
        return Err(AppError::new("当前 Provider 已停用"));
    }

    log_event(
        "provider",
        "build_started",
        serde_json::json!({
            "providerId": record.id.as_str(),
            "providerType": record.provider_type.as_str(),
            "endpoint": record.endpoint.as_str(),
            "model": record.model.as_str(),
        }),
    );

    match record.provider_type {
        ProviderType::OpenAiCompatible => Ok(Box::new(OpenAiCompatibleProvider::new(record))),
        ProviderType::Anthropic => Ok(Box::new(AnthropicProvider::new(record))),
    }
}
