use tauri::State;

use crate::agent::prompt::build_system_prompt;
use crate::commands::settings::get_settings;
use crate::error::IntoCommandResult;
use crate::harness::compression::estimate_text_tokens;
use crate::llm::model::configured_context_window;
use crate::session::manager::{SessionMessage, SessionRecord};
use crate::session::message_mutations::rewrite_message;
use crate::state::AppState;

#[tauri::command]
pub fn session_list(state: State<'_, AppState>) -> Result<Vec<SessionRecord>, String> {
    let mut sessions = state.sessions.list().into_command_result()?;
    let default_provider = state.providers.get_default().into_command_result()?;
    let settings = get_settings(&state).into_command_result()?;
    for session in &mut sessions {
        if let Some(agent) = state.agents.get(&session.agent_id).into_command_result()? {
            session.context_tokens_max = configured_context_window(
                &settings.context_window_overrides,
                default_provider.as_ref(),
                &agent.model,
            )
            .unwrap_or(
                state
                    .providers
                    .resolve_context_window(default_provider.as_ref(), &agent.model)
                    .into_command_result()?,
            );
            session.context_tokens_used = estimate_current_context_tokens(&state, session, &agent)?;
        }
    }
    Ok(sessions)
}

#[tauri::command]
pub fn session_create(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<SessionRecord, String> {
    let agent = state
        .agents
        .get(&agent_id)
        .into_command_result()?
        .ok_or_else(|| "Agent 不存在".to_string())?;
    let default_provider = state.providers.get_default().into_command_result()?;
    let settings = get_settings(&state).into_command_result()?;
    state
        .sessions
        .create_with_context_max(
            agent_id,
            None,
            configured_context_window(
                &settings.context_window_overrides,
                default_provider.as_ref(),
                &agent.model,
            )
            .unwrap_or(
                state
                    .providers
                    .resolve_context_window(default_provider.as_ref(), &agent.model)
                    .into_command_result()?,
            ),
        )
        .into_command_result()
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

#[tauri::command]
pub fn session_rewrite_message(
    state: State<'_, AppState>,
    session_id: String,
    message_id: String,
    content: String,
) -> Result<(), String> {
    rewrite_message(&state.db, &session_id, &message_id, &content).into_command_result()
}

fn estimate_current_context_tokens(
    state: &AppState,
    session: &SessionRecord,
    agent: &crate::agent::definition::AgentRecord,
) -> Result<usize, String> {
    let skill_prompt = state.skills.active_instructions().into_command_result()?;
    let tool_schemas = state.tools.list();
    let system_prompt = build_system_prompt(agent, &skill_prompt, "", &tool_schemas);
    let mut total = estimate_text_tokens(&system_prompt);
    let messages = state.sessions.messages(&session.id).into_command_result()?;
    for message in messages {
        total += estimate_text_tokens(&message.content);
    }
    Ok(total)
}
