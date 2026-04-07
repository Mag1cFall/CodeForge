use crate::llm::model::ChatMessage;

const CHARS_PER_TOKEN: usize = 4;
const MESSAGE_OVERHEAD_TOKENS: usize = 1;
const TRUNCATED_MARKER: &str = "\n[TRUNCATED]";
const SUMMARY_PREVIEW_CHARS: usize = 180;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionResult {
    pub summary: String,
    pub messages: Vec<ChatMessage>,
}

pub fn estimate_text_tokens(text: &str) -> usize {
    let char_count = text.chars().count();
    if char_count == 0 {
        0
    } else {
        (char_count + CHARS_PER_TOKEN - 1) / CHARS_PER_TOKEN
    }
}

pub fn estimate_messages_tokens(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|message| estimate_text_tokens(&message.content) + MESSAGE_OVERHEAD_TOKENS)
        .sum()
}

pub fn compress_messages(
    messages: &[ChatMessage],
    max_tokens: usize,
    keep_recent: usize,
) -> CompressionResult {
    if messages.is_empty() || max_tokens == 0 {
        log_compression_event(
            "compress_skip_empty",
            serde_json::json!({
                "inputMessages": messages.len(),
                "maxTokens": max_tokens,
            }),
        );
        return CompressionResult {
            summary: String::new(),
            messages: vec![],
        };
    }

    let keep_recent = keep_recent.max(1);
    let filtered = prune_old_tool_results(messages, keep_recent);
    let estimated_before = estimate_messages_tokens(&filtered);

    if estimated_before <= max_tokens {
        log_compression_event(
            "compress_skip_under_budget",
            serde_json::json!({
                "inputMessages": messages.len(),
                "filteredMessages": filtered.len(),
                "estimatedTokens": estimated_before,
                "maxTokens": max_tokens,
            }),
        );
        return CompressionResult {
            summary: String::new(),
            messages: filtered,
        };
    }

    let (system_messages, historical_messages, mut recent_messages) =
        split_history(&filtered, keep_recent);

    while recent_messages.len() > 1 {
        let candidate = merge_messages(&system_messages, &recent_messages);
        if estimate_messages_tokens(&candidate) <= max_tokens {
            break;
        }
        recent_messages.remove(0);
    }

    let preserved_messages = merge_messages(&system_messages, &recent_messages);
    let preserved_tokens = estimate_messages_tokens(&preserved_messages);
    let summary_budget = max_tokens.saturating_sub(preserved_tokens);
    let mut summary = summarize_messages(&historical_messages, summary_budget);

    if estimate_text_tokens(&summary) > summary_budget {
        summary = truncate_to_token_budget(&summary, summary_budget);
    }

    log_compression_event(
        "compress_complete",
        serde_json::json!({
            "inputMessages": messages.len(),
            "filteredMessages": filtered.len(),
            "historicalMessages": historical_messages.len(),
            "preservedMessages": preserved_messages.len(),
            "estimatedBefore": estimated_before,
            "preservedTokens": preserved_tokens,
            "summaryTokens": estimate_text_tokens(&summary),
            "maxTokens": max_tokens,
        }),
    );

    CompressionResult {
        summary,
        messages: preserved_messages,
    }
}

fn prune_old_tool_results(messages: &[ChatMessage], keep_recent: usize) -> Vec<ChatMessage> {
    let recent_start = messages.len().saturating_sub(keep_recent);
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            if index < recent_start && is_tool_result_message(message) {
                ChatMessage {
                    role: message.role.clone(),
                    content: format!(
                        "Tool result:\n[omitted {} chars]",
                        message.content.chars().count()
                    ),
                }
            } else {
                message.clone()
            }
        })
        .collect()
}

fn is_tool_result_message(message: &ChatMessage) -> bool {
    message.role == "assistant" && message.content.starts_with("Tool result:")
}

fn split_history(
    messages: &[ChatMessage],
    keep_recent: usize,
) -> (Vec<ChatMessage>, Vec<ChatMessage>, Vec<ChatMessage>) {
    let first_non_system = messages
        .iter()
        .position(|message| message.role != "system")
        .unwrap_or(messages.len());
    let system_messages = messages[..first_non_system].to_vec();
    let non_system_messages = &messages[first_non_system..];

    if non_system_messages.len() <= keep_recent {
        return (system_messages, Vec::new(), non_system_messages.to_vec());
    }

    let split_index = non_system_messages.len().saturating_sub(keep_recent);
    (
        system_messages,
        non_system_messages[..split_index].to_vec(),
        non_system_messages[split_index..].to_vec(),
    )
}

fn merge_messages(
    system_messages: &[ChatMessage],
    recent_messages: &[ChatMessage],
) -> Vec<ChatMessage> {
    let mut merged = Vec::with_capacity(system_messages.len() + recent_messages.len());
    merged.extend(system_messages.iter().cloned());
    merged.extend(recent_messages.iter().cloned());
    merged
}

fn summarize_messages(messages: &[ChatMessage], token_budget: usize) -> String {
    if messages.is_empty() || token_budget == 0 {
        return String::new();
    }

    let mut used = 0usize;
    let mut lines = Vec::new();
    let prioritized = messages
        .iter()
        .filter(|message| is_tool_result_message(message))
        .chain(
            messages
                .iter()
                .filter(|message| !is_tool_result_message(message)),
        );

    for message in prioritized {
        let line = summarize_message_line(message);
        if line.is_empty() {
            continue;
        }

        let tokens = estimate_text_tokens(&line);
        if used + tokens > token_budget && !lines.is_empty() {
            break;
        }

        if used + tokens > token_budget {
            lines.push(truncate_to_token_budget(
                &line,
                token_budget.saturating_sub(used),
            ));
            break;
        }

        used += tokens;
        lines.push(line);
    }

    lines.join("\n")
}

fn summarize_message_line(message: &ChatMessage) -> String {
    if is_tool_result_message(message) {
        return format!(
            "[{}] Tool result omitted ({} chars)",
            message.role,
            message.content.chars().count()
        );
    }

    let compact = message
        .content
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if compact.is_empty() {
        return String::new();
    }

    let preview = if compact.chars().count() > SUMMARY_PREVIEW_CHARS {
        format!(
            "{}…",
            compact
                .chars()
                .take(SUMMARY_PREVIEW_CHARS)
                .collect::<String>()
        )
    } else {
        compact
    };
    format!("[{}] {}", message.role, preview)
}

fn truncate_to_token_budget(content: &str, max_tokens: usize) -> String {
    if content.is_empty() || max_tokens == 0 {
        return String::new();
    }

    let max_chars = max_tokens * CHARS_PER_TOKEN;
    if content.chars().count() <= max_chars {
        return content.to_string();
    }

    let sliced = content.chars().take(max_chars).collect::<String>();
    if let Some(last_newline) = sliced.rfind('\n') {
        if last_newline > 0 {
            return format!("{}{}", &sliced[..last_newline], TRUNCATED_MARKER);
        }
    }

    format!("{}{}", sliced, TRUNCATED_MARKER)
}

fn log_compression_event(event: &str, payload: serde_json::Value) {
    eprintln!(
        "{}",
        serde_json::json!({
            "component": "harness.compression",
            "event": event,
            "payload": payload,
        })
    );
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
        assert!(result.summary.contains("Tool result omitted"));
    }

    #[test]
    fn keeps_system_messages_when_compressing() {
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "system prompt".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: "a".repeat(200),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "b".repeat(200),
            },
            ChatMessage {
                role: "user".into(),
                content: "c".repeat(200),
            },
        ];

        let result = compress_messages(&messages, 80, 1);
        assert!(result
            .messages
            .first()
            .is_some_and(|msg| msg.role == "system"));
        assert!(!result.summary.is_empty());
    }
}
