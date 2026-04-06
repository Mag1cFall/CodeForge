use std::collections::BTreeMap;

use tauri::State;

use crate::error::{AppResult, IntoCommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub language: String,
    pub project_path: Option<String>,
    pub skills_path: Option<String>,
    #[serde(default)]
    pub context_window_overrides: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            language: "zh-CN".into(),
            project_path: None,
            skills_path: None,
            context_window_overrides: BTreeMap::new(),
        }
    }
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Result<AppSettings, String> {
    get_settings(&state).into_command_result()
}

#[tauri::command]
pub fn settings_update(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    let settings = sanitize_settings(settings);
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

#[tauri::command]
pub fn embedding_config_get(state: State<'_, AppState>) -> Result<EmbeddingConfig, String> {
    get_embedding_config(&state).into_command_result()
}

pub(crate) fn get_settings(state: &AppState) -> AppResult<AppSettings> {
    if let Some(value) = state.db.get_json("app_settings")? {
        let mut settings: AppSettings = serde_json::from_str(&value)?;
        if settings.skills_path.is_none() {
            settings.skills_path = Some(state.config.skills_dir.display().to_string());
        }
        return Ok(settings);
    }
    Ok(AppSettings {
        skills_path: Some(state.config.skills_dir.display().to_string()),
        ..AppSettings::default()
    })
}

fn sanitize_settings(mut settings: AppSettings) -> AppSettings {
    settings.context_window_overrides = settings
        .context_window_overrides
        .into_iter()
        .filter_map(|(key, value)| {
            let normalized = key.trim().to_ascii_lowercase();
            if normalized.is_empty() || value == 0 {
                return None;
            }
            Some((normalized, value))
        })
        .collect();
    settings
}

fn get_embedding_config(state: &AppState) -> AppResult<EmbeddingConfig> {
    let provider = state.providers.get_default()?;
    Ok(EmbeddingConfig {
        endpoint: std::env::var("EMBEDDING_API_BASE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| provider.as_ref().map(|item| item.endpoint.clone()))
            .unwrap_or_default(),
        model: std::env::var("EMBEDDING_MODEL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| provider.as_ref().map(|item| item.model.clone()))
            .unwrap_or_default(),
        api_key: std::env::var("EMBEDDING_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| provider.and_then(|item| item.api_key)),
    })
}
