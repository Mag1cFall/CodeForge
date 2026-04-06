use tauri::State;

use crate::error::IntoCommandResult;
use crate::session::manager::{SessionMessage, SessionRecord};
use crate::state::AppState;

#[tauri::command]
pub fn session_list(state: State<'_, AppState>) -> Result<Vec<SessionRecord>, String> {
    state.sessions.list().into_command_result()
}

#[tauri::command]
pub fn session_create(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<SessionRecord, String> {
    state.sessions.create(agent_id, None).into_command_result()
}

#[tauri::command]
pub fn session_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.sessions.delete(&id).into_command_result()
}

#[tauri::command]
pub fn session_messages(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<SessionMessage>, String> {
    state.sessions.messages(&id).into_command_result()
}
