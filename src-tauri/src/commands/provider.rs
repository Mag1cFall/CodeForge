use tauri::State;

use crate::error::IntoCommandResult;
use crate::llm::model::{ProviderConfigInput, ProviderSummary};
use crate::state::AppState;

#[tauri::command]
pub fn provider_list(state: State<'_, AppState>) -> Result<Vec<ProviderSummary>, String> {
    state.providers.list().into_command_result()
}

#[tauri::command]
pub fn provider_create(
    state: State<'_, AppState>,
    config: ProviderConfigInput,
) -> Result<ProviderSummary, String> {
    let provider = state.providers.create(config).into_command_result()?;
    state
        .logs
        .record(
            "provider_create",
            serde_json::json!({ "id": provider.id, "name": provider.name }),
        )
        .map_err(|error| error.message)?;
    Ok(provider)
}

#[tauri::command]
pub fn provider_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.providers.delete(&id).into_command_result()?;
    state
        .logs
        .record("provider_delete", serde_json::json!({ "id": id }))
        .map_err(|error| error.message)?;
    Ok(())
}
