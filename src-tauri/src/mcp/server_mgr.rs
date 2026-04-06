use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::sqlite::Database;
use crate::error::{AppError, AppResult};

use super::client::{McpClient, McpResourceInfo, McpToolInfo};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRecord {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub url: Option<String>,
    pub args: Vec<String>,
    pub env: std::collections::BTreeMap<String, String>,
    pub headers: std::collections::BTreeMap<String, String>,
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
    pub env: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
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
            Ok(McpServerRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                transport: row.get(2)?,
                command: row.get(3)?,
                url: row.get(4)?,
                args: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                env: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                headers: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                enabled: row.get::<_, i64>(8)? != 0,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        let mut servers = Vec::new();
        for row in rows {
            servers.push(row?);
        }
        Ok(servers)
    }

    pub fn add(&self, input: McpServerConfigInput) -> AppResult<McpServerRecord> {
        if input.transport != "stdio" {
            return Err(AppError::new("当前实现仅支持 stdio MCP"));
        }
        let command = input
            .command
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::new("stdio MCP 缺少 command"))?;

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
                input.name,
                input.transport,
                command,
                input.url,
                serde_json::to_string(&input.args)?,
                serde_json::to_string(&input.env)?,
                serde_json::to_string(&input.headers)?,
                if input.enabled { 1 } else { 0 },
                now,
                now,
            ],
        )?;

        self.get(&id)?
            .ok_or_else(|| AppError::new("新建 MCP 记录后未能读取结果"))
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
                Ok(McpServerRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    transport: row.get(2)?,
                    command: row.get(3)?,
                    url: row.get(4)?,
                    args: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                    env: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                    headers: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                    enabled: row.get::<_, i64>(8)? != 0,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .optional()?;
        Ok(server)
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let connection = self.db.connection()?;
        connection.execute("DELETE FROM mcp_servers WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_tools(&self, id: &str) -> AppResult<Vec<McpToolInfo>> {
        let server = self
            .get(id)?
            .ok_or_else(|| AppError::new("指定 MCP Server 不存在"))?;
        let client = McpClient::new(
            server
                .command
                .ok_or_else(|| AppError::new("MCP command 缺失"))?,
            server.args,
        );
        client.list_tools()
    }

    pub fn list_resources(&self, id: &str) -> AppResult<Vec<McpResourceInfo>> {
        let server = self
            .get(id)?
            .ok_or_else(|| AppError::new("指定 MCP Server 不存在"))?;
        let client = McpClient::new(
            server
                .command
                .ok_or_else(|| AppError::new("MCP command 缺失"))?,
            server.args,
        );
        client.list_resources()
    }

    pub fn read_resource(&self, id: &str, uri: &str) -> AppResult<serde_json::Value> {
        let server = self
            .get(id)?
            .ok_or_else(|| AppError::new("指定 MCP Server 不存在"))?;
        let client = McpClient::new(
            server
                .command
                .ok_or_else(|| AppError::new("MCP command 缺失"))?,
            server.args,
        );
        client.read_resource(uri)
    }
}
