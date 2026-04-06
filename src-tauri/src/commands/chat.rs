use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

use crate::agent::runner::AgentRuntime;
use crate::harness::permission::PermissionRequest;
use crate::commands::settings::{get_settings, AppSettings};
use crate::session::message_mutations::delete_after_message;
use crate::skill::manager::SkillSyncSource;
use crate::state::{AppState, PendingPermissionContext};

#[tauri::command]
pub async fn chat_send<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> Result<(), String> {
    state
        .skills
        .sync_from_dirs(&[
            SkillSyncSource {
                root: &state.config.builtin_skills_dir,
                default_enabled: true,
            },
            SkillSyncSource {
                root: &state.config.skills_dir,
                default_enabled: false,
            },
        ])
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
    let settings = get_settings(&state).map_err(|error| error.message.clone())?;
    let runtime = AgentRuntime {
        agent_store: state.agents.clone(),
        provider_store: state.providers.clone(),
        tool_registry: state.tools.clone(),
        session_manager: state.sessions.clone(),
        permission_manager: state.permission.clone(),
        budget: state.budget.clone(),
        logs: state.logs.clone(),
        context_window_overrides: settings.context_window_overrides.clone(),
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

    if let Some(request) = &result.permission_request {
        save_pending_permission(state.inner(), &session_id, request)?;
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
pub async fn chat_retry<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    session_id: String,
    message_id: Option<String>,
) -> Result<(), String> {
    state
        .skills
        .sync_from_dirs(&[
            SkillSyncSource {
                root: &state.config.builtin_skills_dir,
                default_enabled: true,
            },
            SkillSyncSource {
                root: &state.config.skills_dir,
                default_enabled: false,
            },
        ])
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

    let session_messages = state
        .sessions
        .messages(&session_id)
        .map_err(|error| error.message.clone())?;
    let target_index = message_id
        .as_ref()
        .and_then(|id| session_messages.iter().position(|item| item.id == *id))
        .unwrap_or_else(|| session_messages.len().saturating_sub(1));
    let anchor = session_messages
        .iter()
        .take(target_index + 1)
        .rev()
        .find(|item| item.role == "user")
        .ok_or_else(|| "没有可重试的用户消息".to_string())?;

    delete_after_message(&state.db, &session_id, &anchor.id, false)
        .map_err(|error| error.message.clone())?;

    let workspace_root = load_workspace_root(&state)?;
    let settings = get_settings(&state).map_err(|error| error.message.clone())?;
    let runtime = AgentRuntime {
        agent_store: state.agents.clone(),
        provider_store: state.providers.clone(),
        tool_registry: state.tools.clone(),
        session_manager: state.sessions.clone(),
        permission_manager: state.permission.clone(),
        budget: state.budget.clone(),
        logs: state.logs.clone(),
        context_window_overrides: settings.context_window_overrides.clone(),
    };

    let refreshed_session = state
        .sessions
        .get(&session_id)
        .map_err(|error| error.message.clone())?
        .ok_or_else(|| "会话不存在".to_string())?;
    let result = runtime
        .run_from_session(
            &app,
            &agent,
            &refreshed_session,
            state.skills.active_instructions().map_err(|error| error.message.clone())?,
            workspace_root,
        )
        .await
        .map_err(|error| error.message)?;

    if let Some(request) = &result.permission_request {
        save_pending_permission(state.inner(), &session_id, request)?;
    }

    state
        .logs
        .record(
            "chat_retry",
            serde_json::json!({
                "sessionId": session_id,
                "messageId": message_id,
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
pub async fn permission_respond<R: tauri::Runtime>(
    app: AppHandle<R>,
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

    let pending = state
        .pending_permissions
        .lock()
        .map_err(|_| "待处理权限状态被锁定".to_string())?
        .remove(&request_id);

    if let Some(pending) = pending {
        if approved {
            continue_permission_request(&app, &state, pending).await?;
        } else {
            reject_permission_request(&app, &state, pending)?;
        }
    }

    Ok(())
}

async fn continue_permission_request<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    pending: PendingPermissionContext,
) -> Result<(), String> {
    let workspace_root = load_workspace_root(state)?;
    let tool_output = state
        .tools
        .execute(
            &pending.tool_name,
            pending.args.clone(),
            &crate::tools::registry::ToolExecutionContext {
                workspace_root: workspace_root.clone(),
            },
        )
        .map_err(|error| error.message.clone())?;

    let tool_payload = serde_json::json!({
        "id": pending.request_id,
        "name": pending.tool_name,
        "result": tool_output,
    });
    let summary = format!("Tool result:\n{}", tool_payload["result"].as_str().unwrap_or_default());
    state
        .sessions
        .append_message(&pending.session_id, "assistant", &summary, vec![tool_payload.clone()])
        .map_err(|error| error.message.clone())?;

    let session = state
        .sessions
        .get(&pending.session_id)
        .map_err(|error| error.message.clone())?
        .ok_or_else(|| "会话不存在".to_string())?;
    let agent = state
        .agents
        .get(&session.agent_id)
        .map_err(|error| error.message.clone())?
        .ok_or_else(|| "会话关联的 Agent 不存在".to_string())?;
    let settings = get_settings(state).map_err(|error| error.message.clone())?;
    let runtime = AgentRuntime {
        agent_store: state.agents.clone(),
        provider_store: state.providers.clone(),
        tool_registry: state.tools.clone(),
        session_manager: state.sessions.clone(),
        permission_manager: state.permission.clone(),
        budget: state.budget.clone(),
        logs: state.logs.clone(),
        context_window_overrides: settings.context_window_overrides.clone(),
    };

    let result = runtime
        .run_from_session(
            app,
            &agent,
            &session,
            state.skills.active_instructions().map_err(|error| error.message.clone())?,
            workspace_root,
        )
        .await
        .map_err(|error| error.message)?;

    if let Some(request) = &result.permission_request {
        save_pending_permission(state, &pending.session_id, request)?;
    }

    let combined_tool_results = std::iter::once(tool_payload)
        .chain(result.tool_results.into_iter())
        .collect::<Vec<_>>();

    app.emit(
        "chat_chunk",
        serde_json::json!({
            "sessionId": pending.session_id,
            "delta": result.content,
            "done": true,
            "toolResults": combined_tool_results,
        }),
    )
    .map_err(|error| error.to_string())?;

    Ok(())
}

fn reject_permission_request<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    pending: PendingPermissionContext,
) -> Result<(), String> {
    let content = format!("权限已拒绝，未执行工具 {}。", pending.tool_name);
    state
        .sessions
        .append_message(&pending.session_id, "assistant", &content, vec![])
        .map_err(|error| error.message.clone())?;
    app.emit(
        "chat_chunk",
        serde_json::json!({
            "sessionId": pending.session_id,
            "delta": content,
            "done": true,
            "toolResults": [],
        }),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn save_pending_permission(
    state: &AppState,
    session_id: &str,
    request: &PermissionRequest,
) -> Result<(), String> {
    state
        .pending_permissions
        .lock()
        .map_err(|_| "待处理权限状态被锁定".to_string())?
        .insert(
            request.id.clone(),
            PendingPermissionContext {
                request_id: request.id.clone(),
                session_id: session_id.to_string(),
                tool_name: request.tool_name.clone(),
                args: request.args.clone(),
            },
        );

    state
        .db
        .connection()
        .map_err(|error| error.message.clone())?
        .execute(
            r#"
            INSERT INTO permission_requests (id, tool_name, args_json, risk_level, description, status, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?6)
            ON CONFLICT(id) DO UPDATE SET
                tool_name = excluded.tool_name,
                args_json = excluded.args_json,
                risk_level = excluded.risk_level,
                description = excluded.description,
                status = 'pending',
                updated_at = excluded.updated_at
            "#,
            rusqlite::params![
                request.id,
                request.tool_name,
                serde_json::to_string(&request.args).map_err(|error| error.to_string())?,
                serde_json::to_string(&request.risk_level)
                    .map_err(|error| error.to_string())?
                    .trim_matches('"')
                    .to_string(),
                request.description,
                chrono::Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn load_workspace_root(state: &AppState) -> Result<Option<PathBuf>, String> {
    let Some(value) = state.db.get_json("app_settings").map_err(|error| error.message.clone())? else {
        return Ok(None);
    };
    let settings: AppSettings = serde_json::from_str(&value).map_err(|error| error.to_string())?;
    Ok(settings.project_path.map(PathBuf::from))
}
