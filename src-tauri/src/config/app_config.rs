use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub skills_dir: PathBuf,
    pub builtin_skills_dir: PathBuf,
    pub sandbox_root: PathBuf,
}

impl AppConfig {
    pub fn from_app<R: Runtime>(app: &AppHandle<R>) -> AppResult<Self> {
        let data_dir = std::env::var("CODEFORGE_DATA_DIR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or(
                app.path()
                    .app_data_dir()
                    .map_err(|error| AppError::new(error.to_string()))?,
            );

        let skills_dir = resolve_skills_dir(&data_dir);
        let builtin_skills_dir = data_dir.join("builtin-skills");
        let sandbox_root = data_dir.join("sandbox");
        let db_path = data_dir.join("codeforge.db");

        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&skills_dir)?;
        fs::create_dir_all(&builtin_skills_dir)?;
        fs::create_dir_all(&sandbox_root)?;

        Ok(Self {
            data_dir,
            db_path,
            skills_dir,
            builtin_skills_dir,
            sandbox_root,
        })
    }
}

fn resolve_skills_dir(data_dir: &std::path::Path) -> PathBuf {
    let Some(home_dir) = home_dir() else {
        return data_dir.join("skills");
    };

    let candidates = vec![
        home_dir.join(".skills"),
        home_dir.join(".claude").join("skills"),
        home_dir.join(".config").join("opencode").join("skills"),
        home_dir.join(".codex").join("skills"),
    ];

    candidates
        .iter()
        .find(|path| path.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone())
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}
