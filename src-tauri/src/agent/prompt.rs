use crate::tools::schema::ToolSchema;

use super::definition::AgentRecord;

pub fn build_system_prompt(
    agent: &AgentRecord,
    skill_instructions: &str,
    context_summary: &str,
    tools: &[ToolSchema],
) -> String {
    let mut sections = Vec::new();

    if let Some(instructions) = agent
        .instructions
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        sections.push(instructions.trim().to_string());
    }
    if !skill_instructions.trim().is_empty() {
        sections.push(format!(
            "[Skill Instructions]\n{}",
            skill_instructions.trim()
        ));
    }
    if !context_summary.trim().is_empty() {
        sections.push(format!("[Context Summary]\n{}", context_summary.trim()));
    }
    if !tools.is_empty() {
        let tool_text = tools
            .iter()
            .map(|tool| format!("- {}: {}", tool.name, tool.description))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("[Available Tools]\n{}", tool_text));
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::definition::{AgentRecord, AgentStatus};
    use crate::agent::hooks::AgentHooksConfig;

    #[test]
    fn merges_prompt_sources() {
        let prompt = build_system_prompt(
            &AgentRecord {
                id: "agent-1".into(),
                name: "Main".into(),
                instructions: Some("Base instructions".into()),
                tools: vec!["read_file".into()],
                model: "gpt-5.4-mini".into(),
                hooks: AgentHooksConfig::default(),
                status: AgentStatus::Idle,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
            "Skill rules",
            "Summary",
            &[ToolSchema {
                name: "read_file".into(),
                description: "Read file".into(),
                parameters: serde_json::json!({}),
            }],
        );

        assert!(prompt.contains("Base instructions"));
        assert!(prompt.contains("Skill rules"));
        assert!(prompt.contains("read_file"));
    }
}
