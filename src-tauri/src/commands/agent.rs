use tauri::State;

use crate::agent::definition::{AgentConfigInput, AgentRecord, AgentStatus};
use crate::error::IntoCommandResult;
use crate::state::AppState;

#[tauri::command]
pub fn agent_list(state: State<'_, AppState>) -> Result<Vec<AgentRecord>, String> {
    state.agents.list().into_command_result()
}

#[tauri::command]
pub fn agent_create(
    state: State<'_, AppState>,
    config: AgentConfigInput,
) -> Result<AgentRecord, String> {
    let agent = state.agents.create(config).into_command_result()?;
    state
        .logs
        .record(
            "agent_create",
            serde_json::json!({ "id": agent.id, "name": agent.name }),
        )
        .map_err(|error| error.message)?;
    Ok(agent)
}

#[tauri::command]
pub fn agent_update(
    state: State<'_, AppState>,
    id: String,
    config: AgentConfigInput,
) -> Result<AgentRecord, String> {
    let agent = state.agents.update(&id, config).into_command_result()?;
    state
        .logs
        .record(
            "agent_update",
            serde_json::json!({ "id": agent.id, "name": agent.name }),
        )
        .map_err(|error| error.message)?;
    Ok(agent)
}

#[tauri::command]
pub fn agent_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.agents.delete(&id).into_command_result()?;
    state
        .logs
        .record("agent_delete", serde_json::json!({ "id": id }))
        .map_err(|error| error.message)?;
    Ok(())
}

#[tauri::command]
pub fn agent_start(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .agents
        .set_status(&id, AgentStatus::Running)
        .into_command_result()?;
    state
        .logs
        .record("agent_start", serde_json::json!({ "id": id }))
        .map_err(|error| error.message)?;
    Ok(())
}

#[tauri::command]
pub fn agent_stop(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .agents
        .set_status(&id, AgentStatus::Stopped)
        .into_command_result()?;
    state
        .logs
        .record("agent_stop", serde_json::json!({ "id": id }))
        .map_err(|error| error.message)?;
    Ok(())
}
