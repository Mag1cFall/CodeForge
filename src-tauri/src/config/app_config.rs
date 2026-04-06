use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub skills_dir: PathBuf,
    pub sandbox_root: PathBuf,
}

impl AppConfig {
    pub fn from_app<R: Runtime>(app: &AppHandle<R>) -> AppResult<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| AppError::new(error.to_string()))?;

        let skills_dir = data_dir.join("skills");
        let sandbox_root = data_dir.join("sandbox");
        let db_path = data_dir.join("codeforge.db");

        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&skills_dir)?;
        fs::create_dir_all(&sandbox_root)?;

        Ok(Self {
            data_dir,
            db_path,
            skills_dir,
            sandbox_root,
        })
    }
}
