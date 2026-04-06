use crate::llm::model::ChatMessage;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionResult {
    pub summary: String,
    pub messages: Vec<ChatMessage>,
}

pub fn compress_messages(
    messages: &[ChatMessage],
    max_chars: usize,
    keep_recent: usize,
) -> CompressionResult {
    let total_chars = messages
        .iter()
        .map(|message| message.content.len())
        .sum::<usize>();
    if total_chars <= max_chars || messages.len() <= keep_recent {
        return CompressionResult {
            summary: String::new(),
            messages: messages.to_vec(),
        };
    }

    let split_index = messages.len().saturating_sub(keep_recent);
    let summary = messages[..split_index]
        .iter()
        .map(|message| format!("[{}] {}", message.role, message.content.replace('\n', " ")))
        .collect::<Vec<_>>()
        .join("\n");

    CompressionResult {
        summary,
        messages: messages[split_index..].to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_recent_messages_when_over_limit() {
        let messages = vec![
            ChatMessage {
                role: "user".into(),
                content: "a".repeat(20),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "b".repeat(20),
            },
            ChatMessage {
                role: "user".into(),
                content: "c".repeat(20),
            },
        ];

        let result = compress_messages(&messages, 30, 1);
        assert_eq!(result.messages.len(), 1);
        assert!(!result.summary.is_empty());
    }
}
