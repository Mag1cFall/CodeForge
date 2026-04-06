use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use walkdir::WalkDir;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct SandboxManager {
    root: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxWorkspace {
    pub path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

impl SandboxManager {
    pub fn new(root: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn prepare_workspace(&self, source_path: &Path) -> AppResult<SandboxWorkspace> {
        let workspace = self
            .root
            .join(format!("workspace-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace)?;

        for entry in WalkDir::new(source_path) {
            let entry = entry.map_err(|error| AppError::new(error.to_string()))?;
            let path = entry.path();
            if path.starts_with(&self.root) {
                continue;
            }
            let relative = path
                .strip_prefix(source_path)
                .map_err(|error| AppError::new(error.to_string()))?;
            let target = workspace.join(relative);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&target)?;
            } else if entry.file_type().is_file() {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(path, target)?;
            }
        }

        Ok(SandboxWorkspace { path: workspace })
    }

    pub fn run_command(
        &self,
        command: &str,
        workdir: &Path,
        timeout: Duration,
    ) -> AppResult<ShellExecutionResult> {
        let start = Instant::now();
        let output = Command::new("cmd")
            .args(["/C", command])
            .current_dir(workdir)
            .output()?;

        let duration_ms = start.elapsed().as_millis();
        if duration_ms > timeout.as_millis() {
            return Err(AppError::new("Shell 执行超过超时阈值"));
        }

        Ok(ShellExecutionResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_isolated_workspace() {
        let root = std::env::temp_dir().join(format!("codeforge-sandbox-{}", uuid::Uuid::new_v4()));
        let source = root.join("source");
        std::fs::create_dir_all(&source).expect("source dir should exist");
        std::fs::write(source.join("demo.txt"), "hello").expect("source file should exist");

        let manager =
            SandboxManager::new(root.join("sandboxes")).expect("sandbox should initialize");
        let workspace = manager
            .prepare_workspace(&source)
            .expect("workspace should be copied");
        assert!(workspace.path.join("demo.txt").exists());
    }
}
