use std::collections::BTreeMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProviderType {
    OpenAiCompatible,
    Anthropic,
}

impl ProviderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openAiCompatible",
            Self::Anthropic => "anthropic",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderExtra {
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub context_windows: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRecord {
    pub id: String,
    pub name: String,
    pub provider_type: ProviderType,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub model: String,
    pub extra: ProviderExtra,
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSummary {
    pub id: String,
    pub name: String,
    pub provider_type: ProviderType,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub model: String,
    pub models: Vec<String>,
    pub enabled: bool,
    pub key_set: bool,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&ProviderRecord> for ProviderSummary {
    fn from(value: &ProviderRecord) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            provider_type: value.provider_type.clone(),
            endpoint: value.endpoint.clone(),
            api_key: value.api_key.clone(),
            model: value.model.clone(),
            models: value.extra.models.clone(),
            enabled: value.enabled,
            key_set: value
                .api_key
                .as_ref()
                .is_some_and(|key| !key.trim().is_empty()),
            is_default: value.is_default,
            created_at: value.created_at.clone(),
            updated_at: value.updated_at.clone(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigInput {
    pub name: String,
    pub provider_type: ProviderType,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub model: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

fn default_true() -> bool {
    true
}

pub fn model_context_window(model: &str) -> usize {
    let normalized = model.trim().to_ascii_lowercase();
    if normalized.contains("gpt-5.4-mini") {
        return 400_000;
    }
    if normalized.contains("gpt-5.4") {
        return 1_000_000;
    }
    if normalized.contains("claude") {
        return 200_000;
    }
    if normalized.contains("deepseek") || normalized.contains("qwen") {
        return 128_000;
    }
    128_000
}

pub fn configured_context_window(
    overrides: &BTreeMap<String, usize>,
    provider: Option<&ProviderRecord>,
    model: &str,
) -> Option<usize> {
    let normalized_model = normalize_context_key(model);
    if let Some(value) = overrides.get(&normalized_model).copied() {
        return Some(value);
    }

    if let Some(provider) = provider {
        let provider_type_key = format!(
            "{}/{}",
            provider.provider_type.as_str().to_ascii_lowercase(),
            normalized_model
        );
        if let Some(value) = overrides.get(&provider_type_key).copied() {
            return Some(value);
        }

        let provider_name_key = format!(
            "{}/{}",
            normalize_context_key(&provider.name),
            normalized_model
        );
        if let Some(value) = overrides.get(&provider_name_key).copied() {
            return Some(value);
        }
    }

    None
}

pub fn normalize_context_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        configured_context_window, model_context_window, ProviderExtra, ProviderRecord,
        ProviderType,
    };

    #[test]
    fn resolves_known_model_context_windows() {
        assert_eq!(model_context_window("gpt-5.4-mini"), 400_000);
        assert_eq!(model_context_window("gpt-5.4"), 1_000_000);
        assert_eq!(model_context_window("claude-sonnet-4-6"), 200_000);
    }

    #[test]
    fn prefers_configured_context_window_override() {
        let mut overrides = BTreeMap::new();
        overrides.insert("openaicompatible/gpt-5.4-mini".into(), 123_456);
        let provider = ProviderRecord {
            id: "provider-1".into(),
            name: "OpenAI Default".into(),
            provider_type: ProviderType::OpenAiCompatible,
            endpoint: "https://example.com/v1".into(),
            api_key: None,
            model: "gpt-5.4-mini".into(),
            extra: ProviderExtra::default(),
            enabled: true,
            is_default: true,
            created_at: String::new(),
            updated_at: String::new(),
        };

        assert_eq!(
            configured_context_window(&overrides, Some(&provider), "gpt-5.4-mini"),
            Some(123_456)
        );
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub id: Option<String>,
    pub model: String,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunk {
    pub delta: String,
    pub done: bool,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
}
