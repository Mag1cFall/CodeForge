use tauri::State;

use crate::logging::service::TraceLogFilter;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceLog {
    pub id: i64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[tauri::command]
pub fn log_list(state: State<'_, AppState>, limit: usize) -> Result<Vec<TraceLog>, String> {
    Ok(state
        .logs
        .list(TraceLogFilter {
            kind: None,
            limit: Some(limit),
        })
        .map_err(|error| error.message)?
        .into_iter()
        .map(|record| TraceLog {
            id: record.id,
            kind: record.kind,
            payload: record.payload,
            created_at: record.created_at,
        })
        .collect())
}
