use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

use crate::agent::runner::AgentRuntime;
use crate::commands::settings::AppSettings;
use crate::state::AppState;

#[tauri::command]
pub async fn chat_send<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> Result<(), String> {
    state
        .skills
        .sync_from_dir(&state.config.skills_dir)
        .map_err(|error| error.message.clone())?;

    let session = state
        .sessions
        .get(&session_id)
        .map_err(|error| error.message.clone())?
        .ok_or_else(|| "会话不存在".to_string())?;
    let agent = state
        .agents
        .get(&session.agent_id)
        .map_err(|error| error.message.clone())?
        .ok_or_else(|| "会话关联的 Agent 不存在".to_string())?;

    let workspace_root = load_workspace_root(&state)?;
    let runtime = AgentRuntime {
        agent_store: state.agents.clone(),
        provider_store: state.providers.clone(),
        tool_registry: state.tools.clone(),
        session_manager: state.sessions.clone(),
        permission_manager: state.permission.clone(),
        budget: state.budget.clone(),
    };

    let result = runtime
        .run(
            &app,
            &agent,
            &session,
            message,
            state.skills.active_instructions().map_err(|error| error.message.clone())?,
            workspace_root,
        )
        .await
        .map_err(|error| error.message)?;

    if let Some(request) = result.permission_request {
        state
            .db
            .connection()
            .map_err(|error| error.message.clone())?
            .execute(
                r#"
                INSERT INTO permission_requests (id, tool_name, args_json, risk_level, description, status, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?6)
                "#,
                rusqlite::params![
                    request.id,
                    request.tool_name,
                    serde_json::to_string(&request.args).map_err(|error| error.to_string())?,
                    serde_json::to_string(&request.risk_level).map_err(|error| error.to_string())?.trim_matches('"').to_string(),
                    request.description,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|error| error.to_string())?;
    }

    state
        .logs
        .record(
            "chat",
            serde_json::json!({
                "sessionId": session_id,
                "content": result.content,
                "toolResults": result.tool_results,
            }),
        )
        .map_err(|error| error.message.clone())?;

    app.emit(
        "chat_chunk",
        serde_json::json!({
            "sessionId": session_id,
            "delta": result.content,
            "done": true,
            "toolResults": result.tool_results,
        }),
    )
    .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn permission_respond(
    state: State<'_, AppState>,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    let connection = state.db.connection().map_err(|error| error.message.clone())?;
    connection
        .execute(
            "UPDATE permission_requests SET status = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![
                request_id,
                if approved { "approved" } else { "denied" },
                chrono::Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|error| error.to_string())?;
    state
        .logs
        .record(
            "permission_response",
            serde_json::json!({ "requestId": request_id, "approved": approved }),
        )
        .map_err(|error| error.message)?;
    Ok(())
}

fn load_workspace_root(state: &AppState) -> Result<Option<PathBuf>, String> {
    let Some(value) = state.db.get_json("app_settings").map_err(|error| error.message.clone())? else {
        return Ok(None);
    };
    let settings: AppSettings = serde_json::from_str(&value).map_err(|error| error.to_string())?;
    Ok(settings.project_path.map(PathBuf::from))
}
