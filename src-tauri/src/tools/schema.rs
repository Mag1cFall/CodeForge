#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolSchema {
    pub fn openai_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters,
            }
        })
    }

    pub fn anthropic_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "input_schema": self.parameters,
        })
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSet {
    pub items: Vec<ToolSchema>,
}

impl ToolSet {
    pub fn new(items: Vec<ToolSchema>) -> Self {
        Self { items }
    }

    pub fn find(&self, name: &str) -> Option<&ToolSchema> {
        self.items.iter().find(|tool| tool.name == name)
    }

    pub fn openai_schema(&self) -> Vec<serde_json::Value> {
        self.items.iter().map(ToolSchema::openai_schema).collect()
    }

    pub fn anthropic_schema(&self) -> Vec<serde_json::Value> {
        self.items
            .iter()
            .map(ToolSchema::anthropic_schema)
            .collect()
    }

    pub fn descriptions(&self) -> String {
        self.items
            .iter()
            .map(|tool| format!("- {}: {}", tool.name, tool.description))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_to_provider_specific_schemas() {
        let tools = ToolSet::new(vec![ToolSchema {
            name: "read_file".into(),
            description: "Read file".into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" } } }),
        }]);

        assert_eq!(tools.openai_schema().len(), 1);
        assert_eq!(tools.anthropic_schema().len(), 1);
        assert!(tools.descriptions().contains("read_file"));
    }
}
