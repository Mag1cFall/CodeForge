use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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
        log_sandbox_event(
            "sandbox_init",
            serde_json::json!({
                "root": root.display().to_string(),
            }),
        );
        Ok(Self { root })
    }

    pub fn prepare_workspace(&self, source_path: &Path) -> AppResult<SandboxWorkspace> {
        let source_root = std::fs::canonicalize(source_path).map_err(|error| {
            AppError::new(format!(
                "无法访问源工作目录 {}: {}",
                source_path.display(),
                error
            ))
        })?;

        let sandbox_root = std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
        if source_root.starts_with(&sandbox_root) {
            return Err(AppError::new(format!(
                "源工作目录位于沙箱根目录内，拒绝复制: {}",
                source_root.display()
            )));
        }

        let workspace = self
            .root
            .join(format!("workspace-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace)?;

        let mut copied_files = 0usize;
        let mut copied_directories = 0usize;
        let mut skipped_symlinks = 0usize;

        for entry in WalkDir::new(&source_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| !entry.path().starts_with(&sandbox_root))
        {
            let entry = entry.map_err(|error| AppError::new(error.to_string()))?;
            let path = entry.path();
            let relative = path
                .strip_prefix(&source_root)
                .map_err(|error| AppError::new(error.to_string()))?;
            if relative.as_os_str().is_empty() {
                continue;
            }

            let target = workspace.join(relative);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&target)?;
                copied_directories += 1;
            } else if entry.file_type().is_file() {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(path, target)?;
                copied_files += 1;
            } else if entry.file_type().is_symlink() {
                skipped_symlinks += 1;
            }
        }

        log_sandbox_event(
            "prepare_workspace",
            serde_json::json!({
                "sourceRoot": source_root.display().to_string(),
                "workspace": workspace.display().to_string(),
                "copiedFiles": copied_files,
                "copiedDirectories": copied_directories,
                "skippedSymlinks": skipped_symlinks,
            }),
        );

        Ok(SandboxWorkspace { path: workspace })
    }

    pub fn run_command(
        &self,
        command: &str,
        workdir: &Path,
        timeout: Duration,
    ) -> AppResult<ShellExecutionResult> {
        let (result, _) = self.run_command_with_workspace(command, workdir, timeout)?;
        Ok(result)
    }

    pub fn run_command_with_workspace(
        &self,
        command: &str,
        workdir: &Path,
        timeout: Duration,
    ) -> AppResult<(ShellExecutionResult, SandboxWorkspace)> {
        let workspace = self.prepare_workspace(workdir)?;
        let result = self.run_command_in_workspace(command, workdir, timeout, &workspace.path)?;
        Ok((result, workspace))
    }

    fn run_command_in_workspace(
        &self,
        command: &str,
        workdir: &Path,
        timeout: Duration,
        sandboxed_workdir: &Path,
    ) -> AppResult<ShellExecutionResult> {
        log_sandbox_event(
            "run_command_start",
            serde_json::json!({
                "workdir": workdir.display().to_string(),
                "sandboxedWorkdir": sandboxed_workdir.display().to_string(),
                "timeoutMs": timeout.as_millis(),
                "commandPreview": truncate_preview(command, 160),
            }),
        );

        let start = Instant::now();
        let mut child = spawn_shell_process(command, &sandboxed_workdir)?;

        loop {
            if let Some(status) = child.try_wait()? {
                let output = child.wait_with_output()?;
                let result = ShellExecutionResult {
                    exit_code: status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    duration_ms: start.elapsed().as_millis(),
                };

                log_sandbox_event(
                    "run_command_complete",
                    serde_json::json!({
                        "exitCode": result.exit_code,
                        "durationMs": result.duration_ms,
                        "stdoutBytes": result.stdout.len(),
                        "stderrBytes": result.stderr.len(),
                    }),
                );
                return Ok(result);
            }

            if start.elapsed() >= timeout {
                child.kill()?;
                let output = child.wait_with_output()?;
                let result = ShellExecutionResult {
                    exit_code: -1,
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: format!(
                        "timeout after {}s\n{}",
                        timeout.as_secs(),
                        String::from_utf8_lossy(&output.stderr)
                    ),
                    duration_ms: start.elapsed().as_millis(),
                };

                log_sandbox_event(
                    "run_command_timeout",
                    serde_json::json!({
                        "durationMs": result.duration_ms,
                        "timeoutMs": timeout.as_millis(),
                        "stdoutBytes": result.stdout.len(),
                        "stderrBytes": result.stderr.len(),
                    }),
                );
                return Ok(result);
            }

            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn spawn_shell_process(command: &str, workdir: &Path) -> AppResult<std::process::Child> {
    let mut shell = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", command]);
        cmd
    };

    shell
        .current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(shell.spawn()?)
}

fn truncate_preview(input: &str, limit: usize) -> String {
    let mut chars = input.chars();
    let preview = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

fn log_sandbox_event(event: &str, payload: serde_json::Value) {
    eprintln!(
        "{}",
        serde_json::json!({
            "component": "harness.sandbox",
            "event": event,
            "payload": payload,
        })
    );
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

    #[test]
    fn returns_timeout_result_for_long_running_command() {
        let root = std::env::temp_dir().join(format!(
            "codeforge-sandbox-timeout-{}",
            uuid::Uuid::new_v4()
        ));
        let source = root.join("source");
        std::fs::create_dir_all(&source).expect("temp dir should exist");
        let manager =
            SandboxManager::new(root.join("sandboxes")).expect("sandbox should initialize");

        let command = if cfg!(windows) {
            "ping -n 5 127.0.0.1 >nul"
        } else {
            "sleep 5"
        };
        let result = manager
            .run_command(command, &source, Duration::from_millis(100))
            .expect("run_command should return timeout result");
        assert_eq!(result.exit_code, -1);
        assert!(result.stderr.contains("timeout"));
    }
}
