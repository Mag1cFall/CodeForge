use async_trait::async_trait;

use crate::error::{AppError, AppResult};

use super::anthropic::AnthropicProvider;
use super::model::{ChatRequest, ChatResponse, ProviderRecord, ProviderType, StreamChunk};
use super::openai_compatible::OpenAiCompatibleProvider;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> AppResult<ChatResponse>;
    async fn chat_stream(&self, request: ChatRequest) -> AppResult<Vec<StreamChunk>>;
}

pub fn build_provider(record: ProviderRecord) -> AppResult<Box<dyn LlmProvider>> {
    if !record.enabled {
        return Err(AppError::new("当前 Provider 已停用"));
    }

    match record.provider_type {
        ProviderType::OpenAiCompatible => Ok(Box::new(OpenAiCompatibleProvider::new(record))),
        ProviderType::Anthropic => Ok(Box::new(AnthropicProvider::new(record))),
    }
}
