use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::db::sqlite::Database;
use crate::error::{AppError, AppResult};

use super::client::{HttpTransportMode, McpClient, McpResourceInfo, McpToolInfo};
use super::log_structured;

const TRANSPORT_STDIO: &str = "stdio";
const TRANSPORT_SSE: &str = "sse";
const TRANSPORT_STREAMABLE_HTTP: &str = "streamable-http";

type RawServerRow = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    i64,
    String,
    String,
);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRecord {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub url: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfigInput {
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct McpServerManager {
    db: Database,
}

impl McpServerManager {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list(&self) -> AppResult<Vec<McpServerRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, name, transport, command, url, args_json, env_json, headers_json, enabled, created_at, updated_at
            FROM mcp_servers ORDER BY updated_at DESC
            "#,
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
            ))
        })?;

        let mut servers = Vec::new();
        for row in rows {
            servers.push(decode_server_row(row?)?);
        }

        log_structured(
            "mcp.server_mgr",
            "server.list",
            serde_json::json!({ "count": servers.len() }),
        );
        Ok(servers)
    }

    pub fn add(&self, input: McpServerConfigInput) -> AppResult<McpServerRecord> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::new("MCP server 名称不能为空"));
        }

        let transport = canonical_transport(&input.transport)?;
        let (command, args, url) = match transport {
            TRANSPORT_STDIO => {
                let raw_command = input.command.as_deref().unwrap_or("").trim().to_string();
                if raw_command.is_empty() {
                    return Err(AppError::new("stdio MCP 缺少 command"));
                }
                let (command, args) = normalize_stdio_command(&raw_command, input.args.clone())?;
                (Some(command), args, None)
            }
            TRANSPORT_SSE | TRANSPORT_STREAMABLE_HTTP => {
                let url = input.url.as_deref().unwrap_or("").trim().to_string();
                if url.is_empty() {
                    return Err(AppError::new("HTTP MCP 缺少 url"));
                }
                (None, Vec::new(), Some(url))
            }
            _ => unreachable!(),
        };

        log_structured(
            "mcp.server_mgr",
            "server.add.request",
            serde_json::json!({
                "name": name,
                "transport": transport,
                "argCount": args.len(),
                "envKeyCount": input.env.len(),
                "headerKeyCount": input.headers.len(),
                "enabled": input.enabled,
            }),
        );

        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let connection = self.db.connection()?;
        connection.execute(
            r#"
            INSERT INTO mcp_servers (id, name, transport, command, url, args_json, env_json, headers_json, enabled, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                id,
                name,
                transport,
                command,
                url,
                serde_json::to_string(&args)?,
                serde_json::to_string(&input.env)?,
                serde_json::to_string(&input.headers)?,
                if input.enabled { 1 } else { 0 },
                now,
                now,
            ],
        )?;

        let record = self
            .get(&id)?
            .ok_or_else(|| AppError::new("新建 MCP 记录后未能读取结果"))?;
        log_structured(
            "mcp.server_mgr",
            "server.add.success",
            serde_json::json!({
                "id": record.id,
                "name": record.name,
                "transport": record.transport,
            }),
        );
        Ok(record)
    }

    pub fn get(&self, id: &str) -> AppResult<Option<McpServerRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, name, transport, command, url, args_json, env_json, headers_json, enabled, created_at, updated_at
            FROM mcp_servers WHERE id = ?1 LIMIT 1
            "#,
        )?;
        let server = statement
            .query_row(params![id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            })
            .optional()?;

        match server {
            Some(raw) => Ok(Some(decode_server_row(raw)?)),
            None => Ok(None),
        }
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let connection = self.db.connection()?;
        let deleted = connection.execute("DELETE FROM mcp_servers WHERE id = ?1", params![id])?;
        log_structured(
            "mcp.server_mgr",
            "server.remove",
            serde_json::json!({
                "id": id,
                "deleted": deleted > 0,
            }),
        );
        Ok(())
    }

    pub fn list_tools(&self, id: &str) -> AppResult<Vec<McpToolInfo>> {
        let server = self
            .get(id)?
            .ok_or_else(|| AppError::new("指定 MCP Server 不存在"))?;
        ensure_enabled(&server)?;
        let client = build_client_for_server(&server)?;

        log_structured(
            "mcp.server_mgr",
            "server.tools.request",
            serde_json::json!({
                "id": server.id,
                "transport": server.transport,
            }),
        );
        let tools = client.list_tools()?;
        log_structured(
            "mcp.server_mgr",
            "server.tools.success",
            serde_json::json!({
                "id": server.id,
                "toolCount": tools.len(),
            }),
        );
        Ok(tools)
    }

    pub fn list_resources(&self, id: &str) -> AppResult<Vec<McpResourceInfo>> {
        let server = self
            .get(id)?
            .ok_or_else(|| AppError::new("指定 MCP Server 不存在"))?;
        ensure_enabled(&server)?;
        let client = build_client_for_server(&server)?;
        client.list_resources()
    }

    pub fn read_resource(&self, id: &str, uri: &str) -> AppResult<serde_json::Value> {
        let server = self
            .get(id)?
            .ok_or_else(|| AppError::new("指定 MCP Server 不存在"))?;
        ensure_enabled(&server)?;
        let client = build_client_for_server(&server)?;
        client.read_resource(uri)
    }
}

fn ensure_enabled(server: &McpServerRecord) -> AppResult<()> {
    if server.enabled {
        return Ok(());
    }
    Err(AppError::new("MCP Server 已禁用"))
}

fn decode_server_row(raw: RawServerRow) -> AppResult<McpServerRecord> {
    let (
        id,
        name,
        transport,
        command,
        url,
        args_json,
        env_json,
        headers_json,
        enabled,
        created_at,
        updated_at,
    ) = raw;

    let args = decode_json_or_default::<Vec<String>>(&id, "args_json", &args_json);
    let env = decode_json_or_default::<BTreeMap<String, String>>(&id, "env_json", &env_json);
    let headers =
        decode_json_or_default::<BTreeMap<String, String>>(&id, "headers_json", &headers_json);

    Ok(McpServerRecord {
        id,
        name,
        transport,
        command,
        url,
        args,
        env,
        headers,
        enabled: enabled != 0,
        created_at,
        updated_at,
    })
}

fn decode_json_or_default<T>(server_id: &str, field: &str, raw_json: &str) -> T
where
    T: DeserializeOwned + Default,
{
    match serde_json::from_str(raw_json) {
        Ok(value) => value,
        Err(error) => {
            log_structured(
                "mcp.server_mgr",
                "server.config.decode_failed",
                serde_json::json!({
                    "serverId": server_id,
                    "field": field,
                    "error": error.to_string(),
                }),
            );
            T::default()
        }
    }
}

fn normalize_transport(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', "-")
}

fn canonical_transport(value: &str) -> AppResult<&'static str> {
    match normalize_transport(value).as_str() {
        TRANSPORT_STDIO => Ok(TRANSPORT_STDIO),
        TRANSPORT_SSE => Ok(TRANSPORT_SSE),
        "http" | TRANSPORT_STREAMABLE_HTTP => Ok(TRANSPORT_STREAMABLE_HTTP),
        other => Err(AppError::new(format!(
            "不支持的 MCP transport: {other}，仅支持 stdio / sse / streamable-http"
        ))),
    }
}

fn normalize_stdio_command(command: &str, args: Vec<String>) -> AppResult<(String, Vec<String>)> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(AppError::new("stdio MCP 缺少 command"));
    }

    if !args.is_empty() {
        return Ok((trimmed.to_string(), args));
    }

    let mut tokens = split_command_line(trimmed)?;
    let executable = tokens
        .drain(..1)
        .next()
        .ok_or_else(|| AppError::new("stdio MCP command 无法解析可执行程序"))?;
    Ok((executable, tokens))
}

fn build_client_for_server(server: &McpServerRecord) -> AppResult<McpClient> {
    match canonical_transport(&server.transport)? {
        TRANSPORT_STDIO => {
            let raw_command = server
                .command
                .clone()
                .ok_or_else(|| AppError::new("MCP command 缺失"))?;
            let (command, args) = normalize_stdio_command(&raw_command, server.args.clone())?;
            Ok(McpClient::with_stdio(command, args, server.env.clone()))
        }
        TRANSPORT_SSE => {
            let url = server
                .url
                .clone()
                .ok_or_else(|| AppError::new("MCP url 缺失"))?;
            Ok(McpClient::with_http_transport(
                url,
                server.headers.clone(),
                HttpTransportMode::Sse,
            ))
        }
        TRANSPORT_STREAMABLE_HTTP => {
            let url = server
                .url
                .clone()
                .ok_or_else(|| AppError::new("MCP url 缺失"))?;
            Ok(McpClient::with_http_transport(
                url,
                server.headers.clone(),
                HttpTransportMode::StreamableHttp,
            ))
        }
        _ => unreachable!(),
    }
}

fn split_command_line(input: &str) -> AppResult<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escape = false;

    for ch in input.chars() {
        if let Some(q) = quote {
            if escape {
                current.push(ch);
                escape = false;
                continue;
            }
            if q == '"' && ch == '\\' {
                escape = true;
                continue;
            }
            if ch == q {
                quote = None;
                continue;
            }
            current.push(ch);
            continue;
        }

        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }

        if ch.is_whitespace() {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
            continue;
        }

        current.push(ch);
    }

    if escape {
        current.push('\\');
    }

    if quote.is_some() {
        return Err(AppError::new("stdio MCP command 引号不匹配"));
    }

    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        return Err(AppError::new("stdio MCP command 为空"));
    }

    Ok(parts)
}
