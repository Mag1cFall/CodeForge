use tauri::State;

use crate::commands::settings::get_settings;
use crate::error::IntoCommandResult;
use crate::state::AppState;
use crate::tools::schema::ToolSchema;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsageCount {
    pub name: String,
    pub calls: usize,
}

#[tauri::command]
pub fn tool_list(state: State<'_, AppState>) -> Result<Vec<ToolSchema>, String> {
    Ok(state.tools.list())
}

#[tauri::command]
pub fn tool_execute(
    state: State<'_, AppState>,
    name: String,
    args: serde_json::Value,
) -> Result<String, String> {
    let workspace_root = resolve_tool_workspace_root(&state).ok();
    let result = state
        .tools
        .execute(
            &name,
            args,
            &crate::tools::registry::ToolExecutionContext { workspace_root },
        )
        .into_command_result()?;
    state
        .logs
        .record(
            "tool",
            serde_json::json!({ "name": name, "result": result }),
        )
        .map_err(|error| error.message)?;
    Ok(result)
}

#[tauri::command]
pub fn tool_usage_counts(state: State<'_, AppState>) -> Result<Vec<ToolUsageCount>, String> {
    let connection = state.db.connection().map_err(|error| error.message)?;
    let mut counts = std::collections::BTreeMap::<String, usize>::new();

    let mut message_statement = connection
        .prepare("SELECT tool_calls_json FROM messages WHERE tool_calls_json IS NOT NULL AND tool_calls_json != '[]'")
        .map_err(|error| error.to_string())?;
    let message_rows = message_statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;

    for row in message_rows {
        let raw = row.map_err(|error| error.to_string())?;
        let items = serde_json::from_str::<Vec<serde_json::Value>>(&raw).unwrap_or_default();
        for item in items {
            if let Some(name) = item.get("name").and_then(|value| value.as_str()) {
                *counts.entry(name.to_string()).or_insert(0) += 1;
            }
        }
    }

    let mut tool_log_statement = connection
        .prepare("SELECT payload_json FROM logs WHERE kind = 'tool'")
        .map_err(|error| error.to_string())?;
    let tool_log_rows = tool_log_statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    for row in tool_log_rows {
        let raw = row.map_err(|error| error.to_string())?;
        let payload = serde_json::from_str::<serde_json::Value>(&raw).unwrap_or_default();
        if let Some(name) = payload.get("name").and_then(|value| value.as_str()) {
            *counts.entry(name.to_string()).or_insert(0) += 1;
        }
    }

    Ok(counts
        .into_iter()
        .map(|(name, calls)| ToolUsageCount { name, calls })
        .collect())
}

fn resolve_tool_workspace_root(state: &AppState) -> Result<std::path::PathBuf, String> {
    let settings = get_settings(state).map_err(|error| error.message)?;
    if let Some(project_path) = settings
        .project_path
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(std::path::PathBuf::from(project_path));
    }
    std::env::current_dir().map_err(|error| error.to_string())
}
