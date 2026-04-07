use crate::harness::compression::{compress_messages, estimate_messages_tokens, CompressionResult};
use crate::llm::model::ChatMessage;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextWindow {
    pub max_chars: usize,
    pub keep_recent: usize,
}

impl Default for ContextWindow {
    fn default() -> Self {
        Self {
            max_chars: 32_000,
            keep_recent: 16,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnapshot {
    pub total_messages: usize,
    pub estimated_tokens: usize,
    pub summary: String,
    pub recent_messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone)]
pub struct AgentContextManager {
    window: ContextWindow,
}

impl AgentContextManager {
    pub fn new(window: ContextWindow) -> Self {
        Self { window }
    }

    pub fn estimate_tokens(&self, messages: &[ChatMessage]) -> usize {
        estimate_messages_tokens(messages)
    }

    pub fn snapshot(&self, messages: &[ChatMessage]) -> ContextSnapshot {
        let total_messages = messages.len();
        let CompressionResult { summary, messages } =
            compress_messages(messages, self.window.max_chars, self.window.keep_recent);

        ContextSnapshot {
            total_messages,
            estimated_tokens: self.estimate_tokens(messages.as_slice()),
            summary,
            recent_messages: messages,
        }
    }

    pub fn append_tool_summary(
        &self,
        messages: &mut Vec<ChatMessage>,
        tool_name: &str,
        output: &str,
    ) {
        messages.push(ChatMessage {
            role: "assistant".into(),
            content: format!("Tool result:\n[{tool_name}] {output}"),
        });
    }
}

pub fn compress_context(
    messages: &[ChatMessage],
    max_chars: usize,
    keep_recent: usize,
) -> CompressionResult {
    compress_messages(messages, max_chars, keep_recent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_context_snapshot() {
        let manager = AgentContextManager::new(ContextWindow::default());
        let snapshot = manager.snapshot(&vec![
            ChatMessage {
                role: "user".into(),
                content: "hello".into(),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "world".into(),
            },
        ]);

        assert_eq!(snapshot.total_messages, 2);
        assert!(snapshot.estimated_tokens >= 2);
    }
}
