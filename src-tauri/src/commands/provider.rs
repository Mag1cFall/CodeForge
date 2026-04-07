use std::collections::BTreeMap;

use tauri::State;

use crate::error::IntoCommandResult;
use crate::llm::model::{ProviderConfigInput, ProviderSummary, ProviderType};
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
pub fn provider_update(
    state: State<'_, AppState>,
    id: String,
    config: ProviderConfigInput,
) -> Result<ProviderSummary, String> {
    let provider = state
        .providers
        .update(&id, config)
        .into_command_result()?;
    state
        .logs
        .record(
            "provider_update",
            serde_json::json!({ "id": provider.id, "name": provider.name }),
        )
        .map_err(|error| error.message)?;
    Ok(provider)
}

#[tauri::command]
pub async fn provider_fetch_models(
    state: State<'_, AppState>,
    provider_type: ProviderType,
    endpoint: String,
    api_key: Option<String>,
    headers: Option<BTreeMap<String, String>>,
) -> Result<Vec<String>, String> {
    let resolved_headers = headers.unwrap_or_default();
    let models = state
        .providers
        .fetch_models_preview(
            provider_type.clone(),
            endpoint.trim(),
            api_key
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty()),
            &resolved_headers,
        )
        .await
        .into_command_result()?;

    state
        .logs
        .record(
            "provider_fetch_models",
            serde_json::json!({
                "providerType": provider_type.as_str(),
                "endpoint": endpoint,
                "modelCount": models.len(),
            }),
        )
        .map_err(|error| error.message)?;

    Ok(models)
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
