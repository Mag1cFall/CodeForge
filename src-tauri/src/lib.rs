pub mod agent;
pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod harness;
pub mod knowledge;
pub mod llm;
pub mod logging;
pub mod mcp;
pub mod session;
pub mod skill;
pub mod state;
pub mod tools;

#[cfg(all(test, feature = "ipc-tests"))]
mod ipc_tests;

use config::app_config::AppConfig;
use state::AppState;
use tauri::Manager;

pub fn build_app<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .setup(|app| {
            let config = AppConfig::from_app(app.handle()).map_err(|error| error.message.clone())?;
            let state = AppState::new(config).map_err(|error| error.message.clone())?;
            state.initialize_defaults().map_err(|error| error.message.clone())?;
            app.manage(state);
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::chat::chat_send,
            commands::chat::chat_retry,
            commands::session::session_list,
            commands::session::session_create,
            commands::session::session_delete,
            commands::session::session_messages,
            commands::session::session_rewrite_message,
            commands::chat::permission_respond,
            commands::provider::provider_list,
            commands::provider::provider_create,
            commands::provider::provider_delete,
            commands::agent::agent_list,
            commands::agent::agent_create,
            commands::agent::agent_start,
            commands::agent::agent_stop,
            commands::tool::tool_list,
            commands::tool::tool_execute,
            commands::mcp::mcp_server_list,
            commands::mcp::mcp_server_add,
            commands::mcp::mcp_server_remove,
            commands::mcp::mcp_server_tools,
            commands::skill::skill_list,
            commands::skill::skill_toggle,
            commands::knowledge::knowledge_repos,
            commands::knowledge::knowledge_index,
            commands::knowledge::knowledge_search,
            commands::logs::log_list,
            commands::settings::settings_get,
            commands::settings::settings_update,
            commands::settings::embedding_config_get,
            commands::project::project_open,
            commands::project::project_clone,
            commands::project::project_review,
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = dotenvy::dotenv();
    build_app(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
