use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE};

use crate::error::{AppError, AppResult};

use super::log_structured;
use super::transport::exchange_stdio;

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
const MCP_CLIENT_NAME: &str = "codeforge";
const MCP_CLIENT_VERSION: &str = "0.1.0";
const INITIALIZE_ID: i64 = 1;

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
    transport: McpClientTransport,
}

#[derive(Debug, Clone)]
pub(crate) enum HttpTransportMode {
    Sse,
    StreamableHttp,
}

impl HttpTransportMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Sse => "sse",
            Self::StreamableHttp => "streamable-http",
        }
    }
}

#[derive(Debug, Clone)]
enum McpClientTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: BTreeMap<String, String>,
    },
    Http {
        url: String,
        headers: BTreeMap<String, String>,
        mode: HttpTransportMode,
    },
}

impl McpClient {
    pub fn new(command: String, args: Vec<String>) -> Self {
        Self::with_stdio(command, args, BTreeMap::new())
    }

    pub fn with_stdio(command: String, args: Vec<String>, env: BTreeMap<String, String>) -> Self {
        Self {
            transport: McpClientTransport::Stdio { command, args, env },
        }
    }

    pub fn with_http(url: String, headers: BTreeMap<String, String>) -> Self {
        Self::with_http_transport(url, headers, HttpTransportMode::StreamableHttp)
    }

    pub(crate) fn with_http_transport(
        url: String,
        headers: BTreeMap<String, String>,
        mode: HttpTransportMode,
    ) -> Self {
        Self {
            transport: McpClientTransport::Http { url, headers, mode },
        }
    }

    pub fn initialize(&self) -> AppResult<serde_json::Value> {
        self.exchange_with_initialize(INITIALIZE_ID, initialize_request(INITIALIZE_ID))?
            .into_iter()
            .find(|value| value.get("id").and_then(|id| id.as_i64()) == Some(INITIALIZE_ID))
            .ok_or_else(|| AppError::new("MCP initialize 未返回结果"))
    }

    pub fn list_tools(&self) -> AppResult<Vec<McpToolInfo>> {
        let request_id = 2;
        let responses = self.exchange_with_initialize(
            request_id,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "tools/list",
                "params": {}
            }),
        )?;

        let payload = responses
            .into_iter()
            .find(|value| value.get("id").and_then(|id| id.as_i64()) == Some(request_id))
            .ok_or_else(|| AppError::new("MCP tools/list 未返回结果"))?;

        let tools = payload
            .get("result")
            .and_then(|result| result.get("tools"))
            .and_then(|tools| tools.as_array())
            .cloned()
            .unwrap_or_default();

        log_structured(
            "mcp.client",
            "tools.list.success",
            serde_json::json!({
                "transport": self.transport_name(),
                "toolCount": tools.len(),
            }),
        );

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
        let request_id = 3;
        let responses = self.exchange_with_initialize(
            request_id,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "tools/call",
                "params": {
                    "name": name,
                    "arguments": arguments,
                }
            }),
        )?;

        responses
            .into_iter()
            .find(|value| value.get("id").and_then(|id| id.as_i64()) == Some(request_id))
            .ok_or_else(|| AppError::new("MCP tools/call 未返回结果"))
    }

    pub fn list_resources(&self) -> AppResult<Vec<McpResourceInfo>> {
        let request_id = 4;
        let responses = self.exchange_with_initialize(
            request_id,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "resources/list",
                "params": {}
            }),
        )?;

        let payload = responses
            .into_iter()
            .find(|value| value.get("id").and_then(|id| id.as_i64()) == Some(request_id))
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
        let request_id = 5;
        let responses = self.exchange_with_initialize(
            request_id,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "resources/read",
                "params": { "uri": uri }
            }),
        )?;

        responses
            .into_iter()
            .find(|value| value.get("id").and_then(|id| id.as_i64()) == Some(request_id))
            .ok_or_else(|| AppError::new("MCP resources/read 未返回结果"))
    }

    fn exchange_with_initialize(
        &self,
        request_id: i64,
        request: serde_json::Value,
    ) -> AppResult<Vec<serde_json::Value>> {
        let method = request
            .get("method")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string();

        let requests = if request_id == INITIALIZE_ID && method == "initialize" {
            vec![request]
        } else {
            vec![
                initialize_request(INITIALIZE_ID),
                initialized_notification(),
                request,
            ]
        };

        log_structured(
            "mcp.client",
            "exchange.start",
            serde_json::json!({
                "transport": self.transport_name(),
                "requestId": request_id,
                "method": method,
                "messageCount": requests.len(),
            }),
        );

        let responses = match &self.transport {
            McpClientTransport::Stdio { command, args, env } => {
                exchange_stdio(command, args, env, &requests)?
            }
            McpClientTransport::Http { url, headers, mode } => {
                exchange_http(url, headers, mode, &requests)?
            }
        };

        let filtered = responses
            .into_iter()
            .filter(|value| {
                value
                    .get("id")
                    .and_then(|id| id.as_i64())
                    .is_some_and(|id| id == INITIALIZE_ID || id == request_id)
            })
            .collect::<Vec<_>>();

        if !filtered
            .iter()
            .any(|value| value.get("id").and_then(|id| id.as_i64()) == Some(INITIALIZE_ID))
        {
            return Err(AppError::new("MCP initialize 未返回结果"));
        }

        if !filtered
            .iter()
            .any(|value| value.get("id").and_then(|id| id.as_i64()) == Some(request_id))
        {
            return Err(AppError::new(format!("MCP {method} 未返回结果")));
        }

        for response in &filtered {
            ensure_rpc_success(response)?;
        }

        log_structured(
            "mcp.client",
            "exchange.success",
            serde_json::json!({
                "transport": self.transport_name(),
                "requestId": request_id,
                "responseCount": filtered.len(),
            }),
        );

        Ok(filtered)
    }

    fn transport_name(&self) -> &'static str {
        match &self.transport {
            McpClientTransport::Stdio { .. } => "stdio",
            McpClientTransport::Http { mode, .. } => mode.as_str(),
        }
    }
}

fn initialize_request(id: i64) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": MCP_CLIENT_NAME,
                "version": MCP_CLIENT_VERSION,
            }
        }
    })
}

fn initialized_notification() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {}
    })
}

fn exchange_http(
    url: &str,
    headers: &BTreeMap<String, String>,
    mode: &HttpTransportMode,
    requests: &[serde_json::Value],
) -> AppResult<Vec<serde_json::Value>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    if matches!(mode, HttpTransportMode::Sse) {
        preflight_sse_endpoint(&client, url, headers)?;
    }

    let mut responses = Vec::new();
    let mut session_id: Option<String> = None;
    for request in requests {
        let mut builder = client
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream");
        for (key, value) in headers {
            builder = builder.header(key, value);
        }
        if let Some(id) = session_id.as_deref() {
            builder = builder.header("mcp-session-id", id);
        }

        let response = builder.body(serde_json::to_vec(request)?).send()?;
        let response_headers = response.headers().clone();
        let status = response.status();
        let content_type = response_headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string);
        let body = response.text()?;

        if !status.is_success() {
            return Err(AppError::new(format!(
                "MCP HTTP 请求失败: status={status}, body={}",
                compact_text(&body)
            )));
        }

        if session_id.is_none() {
            session_id = extract_session_id(&response_headers);
        }

        responses.extend(parse_http_responses(&body, content_type.as_deref())?);
    }

    if let Some(id) = session_id {
        let mut close_builder = client.delete(url).header("mcp-session-id", &id);
        for (key, value) in headers {
            close_builder = close_builder.header(key, value);
        }
        let _ = close_builder.send();
    }

    Ok(responses)
}

fn preflight_sse_endpoint(
    client: &reqwest::blocking::Client,
    url: &str,
    headers: &BTreeMap<String, String>,
) -> AppResult<()> {
    let mut builder = client
        .get(url)
        .header(ACCEPT, "application/json, text/event-stream");
    for (key, value) in headers {
        builder = builder.header(key, value);
    }

    let response = builder.send()?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(AppError::new(format!(
            "MCP SSE 预检失败: status={status}, body={}",
            compact_text(&body)
        )));
    }

    log_structured(
        "mcp.client",
        "http.sse.preflight.ok",
        serde_json::json!({
            "status": status.as_u16(),
            "url": url,
        }),
    );

    Ok(())
}

fn parse_http_responses(
    body: &str,
    content_type: Option<&str>,
) -> AppResult<Vec<serde_json::Value>> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if content_type
        .map(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false)
    {
        return parse_sse_payload(trimmed);
    }

    parse_json_payload(trimmed)
}

fn parse_json_payload(payload: &str) -> AppResult<Vec<serde_json::Value>> {
    match serde_json::from_str::<serde_json::Value>(payload) {
        Ok(serde_json::Value::Array(values)) => Ok(values),
        Ok(value) => Ok(vec![value]),
        Err(error) => {
            let mut parsed_lines = Vec::new();
            for line in payload
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
                    parsed_lines.push(value);
                }
            }
            if parsed_lines.is_empty() {
                return Err(AppError::new(format!(
                    "MCP 响应 JSON 解析失败: {}; body={} ",
                    error,
                    compact_text(payload)
                )));
            }
            Ok(parsed_lines)
        }
    }
}

fn parse_sse_payload(payload: &str) -> AppResult<Vec<serde_json::Value>> {
    let mut responses = Vec::new();
    let mut data_lines = Vec::new();
    for line in payload.lines().map(str::trim_end) {
        if line.is_empty() {
            flush_sse_event_data(&mut data_lines, &mut responses)?;
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.trim_start().to_string());
        }
    }
    flush_sse_event_data(&mut data_lines, &mut responses)?;
    Ok(responses)
}

fn flush_sse_event_data(
    data_lines: &mut Vec<String>,
    responses: &mut Vec<serde_json::Value>,
) -> AppResult<()> {
    if data_lines.is_empty() {
        return Ok(());
    }

    let event_payload = data_lines.join("\n");
    data_lines.clear();
    let trimmed = event_payload.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return Ok(());
    }

    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(value) => responses.push(value),
        Err(error) => {
            log_structured(
                "mcp.client",
                "http.sse.non_json_event",
                serde_json::json!({
                    "error": error.to_string(),
                    "payload": compact_text(trimmed),
                }),
            );
        }
    }
    Ok(())
}

fn extract_session_id(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get("mcp-session-id")
        .or_else(|| headers.get("x-mcp-session-id"))
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

fn ensure_rpc_success(response: &serde_json::Value) -> AppResult<()> {
    let Some(error) = response.get("error") else {
        return Ok(());
    };

    let code = error
        .get("code")
        .cloned()
        .unwrap_or(serde_json::json!(null));
    let message = error
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown error");
    let data = error
        .get("data")
        .map(|value| compact_text(&value.to_string()))
        .unwrap_or_else(|| "null".to_string());
    Err(AppError::new(format!(
        "MCP 响应错误: code={code}, message={message}, data={data}"
    )))
}

fn compact_text(text: &str) -> String {
    const LIMIT: usize = 500;
    let trimmed = text.trim();
    if trimmed.chars().count() <= LIMIT {
        return trimmed.to_string();
    }
    let preview = trimmed.chars().take(LIMIT).collect::<String>();
    format!("{preview}…")
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
