use tauri::State;

use crate::error::{AppResult, IntoCommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub language: String,
    pub project_path: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            language: "zh-CN".into(),
            project_path: None,
        }
    }
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Result<AppSettings, String> {
    get_settings(&state).into_command_result()
}

#[tauri::command]
pub fn settings_update(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    state
        .db
        .set_json(
            "app_settings",
            &serde_json::to_string(&settings).map_err(|error| error.to_string())?,
            &now,
        )
        .map_err(|error| error.message)
}

fn get_settings(state: &AppState) -> AppResult<AppSettings> {
    if let Some(value) = state.db.get_json("app_settings")? {
        return Ok(serde_json::from_str(&value)?);
    }
    Ok(AppSettings::default())
}
