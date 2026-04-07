use std::path::PathBuf;

use rusqlite::OptionalExtension;
use tauri::{AppHandle, Emitter, State};

use crate::agent::runner::{AgentRunConfig, AgentRuntime};
use crate::harness::permission::PermissionRequest;
use crate::commands::settings::{get_settings, AppSettings};
use crate::session::message_mutations::delete_after_message;
use crate::skill::manager::SkillSyncSource;
use crate::state::{AppState, PendingPermissionContext};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionRunConfig {
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
}

#[tauri::command]
pub async fn chat_send<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    session_id: String,
    message: String,
    config: Option<SessionRunConfig>,
) -> Result<(), String> {
    ensure_skills_ready(state.inner())?;

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
    let runtime_config = config.unwrap_or_default();
    save_session_run_config(state.inner(), &session_id, &runtime_config)?;

    let existing_message_count = state
        .sessions
        .messages(&session_id)
        .map_err(|error| error.message.clone())?
        .len();
    if existing_message_count == 0 {
        let _ = state.sessions.maybe_auto_rename(&session_id, &message);
    }

    let session = state
        .sessions
        .get(&session_id)
        .map_err(|error| error.message.clone())?
        .ok_or_else(|| "会话不存在".to_string())?;

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
            to_agent_run_config(&runtime_config),
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

    emit_chat_stream(&app, &session_id, &result.content, &result.tool_results).await?;

    Ok(())
}

#[tauri::command]
pub async fn chat_retry<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    session_id: String,
    message_id: Option<String>,
) -> Result<(), String> {
    ensure_skills_ready(state.inner())?;

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
    let runtime_config = load_session_run_config(state.inner(), &session_id)?;
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
            to_agent_run_config(&runtime_config),
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

    emit_chat_stream(&app, &session_id, &result.content, &result.tool_results).await?;

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
    let status = connection
        .query_row(
            "SELECT status FROM permission_requests WHERE id = ?1 LIMIT 1",
            rusqlite::params![request_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let status = status.ok_or_else(|| "权限请求不存在或已过期".to_string())?;
    if status != "pending" {
        return Err("权限请求已处理，请勿重复提交".to_string());
    }

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
            reject_permission_request(&app, &state, pending).await?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn permission_pending(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Option<PermissionRequest>, String> {
    let pending = state
        .pending_permissions
        .lock()
        .map_err(|_| "待处理权限状态被锁定".to_string())?
        .values()
        .find(|entry| entry.session_id == session_id)
        .cloned();

    let Some(pending) = pending else {
        return Ok(None);
    };

    let (_, risk_level, description) = state.permission.classify(&pending.tool_name);
    Ok(Some(PermissionRequest {
        id: pending.request_id,
        tool_name: pending.tool_name,
        args: pending.args,
        risk_level,
        description,
    }))
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
        "args": pending.args,
        "result": tool_output,
    });
    app.emit(
        "chat_tool_result",
        serde_json::json!({ "sessionId": pending.session_id, "tool": tool_payload }),
    )
    .map_err(|error| error.to_string())?;
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
    let runtime_config = load_session_run_config(state, &pending.session_id)?;
    let mut agent_run_config = to_agent_run_config(&runtime_config);
    if !agent_run_config
        .approved_tool_names
        .iter()
        .any(|item| item.eq_ignore_ascii_case(&pending.tool_name))
    {
        agent_run_config
            .approved_tool_names
            .push(pending.tool_name.clone());
    }
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
            agent_run_config,
        )
        .await
        .map_err(|error| error.message)?;

    if let Some(request) = &result.permission_request {
        save_pending_permission(state, &pending.session_id, request)?;
    }

    let combined_tool_results = std::iter::once(tool_payload)
        .chain(result.tool_results.into_iter())
        .collect::<Vec<_>>();

    emit_chat_stream(
        app,
        &pending.session_id,
        &result.content,
        &combined_tool_results,
    )
    .await?;

    Ok(())
}

async fn reject_permission_request<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    pending: PendingPermissionContext,
) -> Result<(), String> {
    let content = format!("权限已拒绝，未执行工具 {}。", pending.tool_name);
    state
        .sessions
        .append_message(&pending.session_id, "assistant", &content, vec![])
        .map_err(|error| error.message.clone())?;
    emit_chat_stream(app, &pending.session_id, &content, &[]).await?;
    Ok(())
}

async fn emit_chat_stream<R: tauri::Runtime>(
    app: &AppHandle<R>,
    session_id: &str,
    content: &str,
    tool_results: &[serde_json::Value],
) -> Result<(), String> {
    app.emit(
        "chat_chunk",
        serde_json::json!({
            "sessionId": session_id,
            "delta": content,
            "done": true,
            "toolResults": tool_results,
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

fn save_session_run_config(
    state: &AppState,
    session_id: &str,
    config: &SessionRunConfig,
) -> Result<(), String> {
    state
        .db
        .set_json(
            &format!("session_run_config:{session_id}"),
            &serde_json::to_string(config).map_err(|error| error.to_string())?,
            &chrono::Utc::now().to_rfc3339(),
        )
        .map_err(|error| error.message)
}

fn load_session_run_config(state: &AppState, session_id: &str) -> Result<SessionRunConfig, String> {
    let Some(value) = state
        .db
        .get_json(&format!("session_run_config:{session_id}"))
        .map_err(|error| error.message.clone())?
    else {
        return Ok(SessionRunConfig::default());
    };

    serde_json::from_str(&value).map_err(|error| error.to_string())
}

fn to_agent_run_config(config: &SessionRunConfig) -> AgentRunConfig {
    AgentRunConfig {
        provider_id: config.provider_id.clone(),
        model: config.model.clone(),
        approved_tool_names: Vec::new(),
        temperature: config.temperature,
        top_p: config.top_p,
        max_tokens: config.max_tokens,
    }
}

fn ensure_skills_ready(state: &AppState) -> Result<(), String> {
    let existing = state
        .skills
        .list()
        .map_err(|error| error.message.clone())?;
    if !existing.is_empty() {
        return Ok(());
    }

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

    Ok(())
}
