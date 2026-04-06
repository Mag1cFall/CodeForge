use tauri::State;

use crate::error::IntoCommandResult;
use crate::mcp::{
    client::McpToolInfo,
    server_mgr::{McpServerConfigInput, McpServerRecord},
};
use crate::state::AppState;

#[tauri::command]
pub fn mcp_server_list(state: State<'_, AppState>) -> Result<Vec<McpServerRecord>, String> {
    state.mcp_servers.list().into_command_result()
}

#[tauri::command]
pub fn mcp_server_add(
    state: State<'_, AppState>,
    config: McpServerConfigInput,
) -> Result<McpServerRecord, String> {
    let record = state.mcp_servers.add(config).into_command_result()?;
    state
        .logs
        .record(
            "mcp_add",
            serde_json::json!({ "id": record.id, "name": record.name }),
        )
        .map_err(|error| error.message)?;
    Ok(record)
}

#[tauri::command]
pub fn mcp_server_remove(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.mcp_servers.remove(&id).into_command_result()?;
    state
        .logs
        .record("mcp_remove", serde_json::json!({ "id": id }))
        .map_err(|error| error.message)?;
    Ok(())
}

#[tauri::command]
pub fn mcp_server_tools(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<McpToolInfo>, String> {
    let tools = state.mcp_servers.list_tools(&id).into_command_result()?;
    state
        .logs
        .record(
            "mcp_tools",
            serde_json::json!({ "id": id, "count": tools.len() }),
        )
        .map_err(|error| error.message)?;
    Ok(tools)
}
