use tauri::State;

use crate::commands::settings::get_settings;
use crate::error::IntoCommandResult;
use crate::state::AppState;
use crate::tools::schema::ToolSchema;

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
