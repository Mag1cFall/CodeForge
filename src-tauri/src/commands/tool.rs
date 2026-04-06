use tauri::State;

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
    let result = state
        .tools
        .execute(
            &name,
            args,
            &crate::tools::registry::ToolExecutionContext {
                workspace_root: None,
            },
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
