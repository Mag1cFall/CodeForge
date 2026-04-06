use crate::error::{AppError, AppResult};

use super::transport::exchange_stdio;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceInfo {
    pub uri: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct McpClient {
    command: String,
    args: Vec<String>,
}

impl McpClient {
    pub fn new(command: String, args: Vec<String>) -> Self {
        Self { command, args }
    }

    pub fn initialize(&self) -> AppResult<serde_json::Value> {
        self.exchange_with_initialize(
            1,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "codeforge", "version": "0.1.0" }
                }
            }),
        )?
        .into_iter()
        .find(|value| value.get("id").and_then(|id| id.as_i64()) == Some(1))
        .ok_or_else(|| AppError::new("MCP initialize 未返回结果"))
    }

    pub fn list_tools(&self) -> AppResult<Vec<McpToolInfo>> {
        let responses = self.exchange_with_initialize(
            2,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }),
        )?;

        let payload = responses
            .into_iter()
            .find(|value| value.get("id").and_then(|id| id.as_i64()) == Some(2))
            .ok_or_else(|| AppError::new("MCP tools/list 未返回结果"))?;

        let tools = payload
            .get("result")
            .and_then(|result| result.get("tools"))
            .and_then(|tools| tools.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(tools
            .into_iter()
            .map(|tool| McpToolInfo {
                name: tool
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                description: tool
                    .get("description")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                input_schema: tool
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({})),
            })
            .collect())
    }

    pub fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> AppResult<serde_json::Value> {
        let responses = self.exchange_with_initialize(
            3,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": name,
                    "arguments": arguments,
                }
            }),
        )?;

        responses
            .into_iter()
            .find(|value| value.get("id").and_then(|id| id.as_i64()) == Some(3))
            .ok_or_else(|| AppError::new("MCP tools/call 未返回结果"))
    }

    pub fn list_resources(&self) -> AppResult<Vec<McpResourceInfo>> {
        let responses = self.exchange_with_initialize(
            4,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "resources/list",
                "params": {}
            }),
        )?;

        let payload = responses
            .into_iter()
            .find(|value| value.get("id").and_then(|id| id.as_i64()) == Some(4))
            .ok_or_else(|| AppError::new("MCP resources/list 未返回结果"))?;

        Ok(payload
            .get("result")
            .and_then(|result| result.get("resources"))
            .and_then(|resources| resources.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|resource| McpResourceInfo {
                uri: resource
                    .get("uri")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                name: resource
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                description: resource
                    .get("description")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
            })
            .collect())
    }

    pub fn read_resource(&self, uri: &str) -> AppResult<serde_json::Value> {
        let responses = self.exchange_with_initialize(
            5,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "resources/read",
                "params": { "uri": uri }
            }),
        )?;

        responses
            .into_iter()
            .find(|value| value.get("id").and_then(|id| id.as_i64()) == Some(5))
            .ok_or_else(|| AppError::new("MCP resources/read 未返回结果"))
    }

    fn exchange_with_initialize(
        &self,
        request_id: i64,
        request: serde_json::Value,
    ) -> AppResult<Vec<serde_json::Value>> {
        let responses = exchange_stdio(
            &self.command,
            &self.args,
            &[
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {},
                        "clientInfo": { "name": "codeforge", "version": "0.1.0" }
                    }
                }),
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized",
                    "params": {}
                }),
                request,
            ],
        )?;

        Ok(responses
            .into_iter()
            .filter(|value| {
                value
                    .get("id")
                    .and_then(|id| id.as_i64())
                    .is_some_and(|id| id == 1 || id == request_id)
            })
            .collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_with_fake_stdio_server() {
        let script = r#"import json, sys
for line in sys.stdin:
    req = json.loads(line)
    if req['method'] == 'initialize':
        print(json.dumps({'jsonrpc': '2.0', 'id': req['id'], 'result': {'serverInfo': {'name': 'fake', 'version': '1.0'}}}), flush=True)
    elif req['method'] == 'tools/list':
        print(json.dumps({'jsonrpc': '2.0', 'id': req['id'], 'result': {'tools': [{'name': 'ping', 'description': 'Ping', 'inputSchema': {'type': 'object', 'properties': {}}}]}}), flush=True)
    elif req['method'] == 'tools/call':
        print(json.dumps({'jsonrpc': '2.0', 'id': req['id'], 'result': {'content': [{'type': 'text', 'text': 'pong'}]}}), flush=True)
    elif req['method'] == 'resources/list':
        print(json.dumps({'jsonrpc': '2.0', 'id': req['id'], 'result': {'resources': [{'uri': 'file://demo', 'name': 'demo', 'description': 'Demo resource'}]}}), flush=True)
    elif req['method'] == 'resources/read':
        print(json.dumps({'jsonrpc': '2.0', 'id': req['id'], 'result': {'contents': [{'uri': req['params']['uri'], 'text': 'demo body'}]}}), flush=True)
"#;

        let client = McpClient::new("python".into(), vec!["-c".into(), script.into()]);

        let tools = client.list_tools().expect("tools should list");
        assert_eq!(tools.len(), 1);
        let call = client
            .call_tool("ping", serde_json::json!({}))
            .expect("tool should respond");
        assert_eq!(call["result"]["content"][0]["text"], "pong");
        let resources = client.list_resources().expect("resources should list");
        assert_eq!(resources.len(), 1);
        let resource = client
            .read_resource("file://demo")
            .expect("resource should read");
        assert_eq!(resource["result"]["contents"][0]["text"], "demo body");
    }
}
