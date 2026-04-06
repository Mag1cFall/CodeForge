use tauri::State;

use crate::error::IntoCommandResult;
use crate::skill::loader::SkillRecord;
use crate::state::AppState;

#[tauri::command]
pub fn skill_list(state: State<'_, AppState>) -> Result<Vec<SkillRecord>, String> {
    state
        .skills
        .sync_from_dir(&state.config.skills_dir)
        .into_command_result()?;
    state.skills.list().into_command_result()
}

#[tauri::command]
pub fn skill_toggle(state: State<'_, AppState>, name: String, enabled: bool) -> Result<(), String> {
    state.skills.toggle(&name, enabled).into_command_result()
}
