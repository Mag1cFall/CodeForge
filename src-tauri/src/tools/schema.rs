#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolSchema {
    pub fn openai_schema(&self) -> serde_json::Value {
        let mut function = serde_json::json!({
            "name": self.name,
        });
        if !self.description.trim().is_empty() {
            function["description"] = serde_json::json!(self.description);
        }

        let parameters = normalize_schema(&self.parameters);
        let has_properties = parameters
            .get("properties")
            .and_then(|value| value.as_object())
            .is_some_and(|value| !value.is_empty());
        if has_properties {
            function["parameters"] = parameters;
        }

        serde_json::json!({ "type": "function", "function": function })
    }

    pub fn anthropic_schema(&self) -> serde_json::Value {
        let parameters = normalize_schema(&self.parameters);
        let mut input_schema = serde_json::json!({
            "type": "object",
            "properties": parameters.get("properties").cloned().unwrap_or_else(|| serde_json::json!({})),
            "required": parameters.get("required").cloned().unwrap_or_else(|| serde_json::json!([])),
        });
        if let Some(additional) = parameters.get("additionalProperties") {
            input_schema["additionalProperties"] = additional.clone();
        }

        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "input_schema": input_schema,
        })
    }
}

fn normalize_schema(parameters: &serde_json::Value) -> serde_json::Value {
    let mut schema = if parameters.is_object() {
        parameters.clone()
    } else {
        serde_json::json!({})
    };

    if schema.get("type").is_none() {
        schema["type"] = serde_json::json!("object");
    }
    if schema
        .get("properties")
        .and_then(|value| value.as_object())
        .is_none()
    {
        schema["properties"] = serde_json::json!({});
    }
    if schema
        .get("required")
        .and_then(|value| value.as_array())
        .is_none()
    {
        schema["required"] = serde_json::json!([]);
    }

    schema
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
        assert_eq!(
            tools.anthropic_schema()[0]["input_schema"]["type"],
            "object"
        );
    }
}
