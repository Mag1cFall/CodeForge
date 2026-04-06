use crate::llm::model::ChatMessage;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionResult {
    pub summary: String,
    pub messages: Vec<ChatMessage>,
}

pub fn estimate_text_tokens(text: &str) -> usize {
    text.split_whitespace()
        .count()
        .max(text.chars().count() / 4 + 1)
}

pub fn estimate_messages_tokens(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|message| estimate_text_tokens(&message.content))
        .sum()
}

pub fn compress_messages(
    messages: &[ChatMessage],
    max_tokens: usize,
    keep_recent: usize,
) -> CompressionResult {
    if messages.is_empty() {
        return CompressionResult {
            summary: String::new(),
            messages: vec![],
        };
    }

    let preserved = prune_old_tool_results(messages, keep_recent.max(1));
    if preserved.len() <= keep_recent.max(1) && estimate_messages_tokens(&preserved) <= max_tokens {
        return CompressionResult {
            summary: String::new(),
            messages: preserved,
        };
    }

    let split_index = preserved.len().saturating_sub(keep_recent.max(1));
    let historical = &preserved[..split_index];
    let mut recent = preserved[split_index..].to_vec();

    while recent.len() > 1 && estimate_messages_tokens(&recent) > max_tokens {
        recent.remove(0);
    }

    let summary = summarize_messages(
        historical,
        max_tokens.saturating_sub(estimate_messages_tokens(&recent)),
    );
    CompressionResult {
        summary,
        messages: recent,
    }
}

fn prune_old_tool_results(messages: &[ChatMessage], keep_recent: usize) -> Vec<ChatMessage> {
    let recent_start = messages.len().saturating_sub(keep_recent);
    messages
        .iter()
        .enumerate()
        .filter(|(index, message)| *index >= recent_start || !is_tool_result_message(message))
        .map(|(_, message)| message.clone())
        .collect()
}

fn is_tool_result_message(message: &ChatMessage) -> bool {
    message.role == "assistant" && message.content.starts_with("Tool result:")
}

fn summarize_messages(messages: &[ChatMessage], token_budget: usize) -> String {
    if messages.is_empty() || token_budget == 0 {
        return String::new();
    }

    let mut used = 0usize;
    let mut lines = Vec::new();
    for message in messages {
        let compact = message.content.replace('\n', " ");
        let trimmed = compact.trim();
        if trimmed.is_empty() {
            continue;
        }

        let preview = if trimmed.chars().count() > 140 {
            format!("{}…", trimmed.chars().take(140).collect::<String>())
        } else {
            trimmed.to_string()
        };
        let line = format!("[{}] {}", message.role, preview);
        let tokens = estimate_text_tokens(&line);
        if used + tokens > token_budget && !lines.is_empty() {
            break;
        }
        used += tokens;
        lines.push(line);
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_recent_messages_when_over_limit() {
        let messages = vec![
            ChatMessage {
                role: "user".into(),
                content: "a".repeat(80),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "Tool result:\nvery long output".into(),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "b".repeat(80),
            },
            ChatMessage {
                role: "user".into(),
                content: "c".repeat(80),
            },
        ];

        let result = compress_messages(&messages, 60, 2);
        assert_eq!(result.messages.len(), 2);
        assert!(!result.summary.is_empty());
        assert!(!result.summary.contains("Tool result:"));
    }
}
