use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};
use crate::harness::permission::{PermissionManager, PermissionPolicy};
use crate::harness::sandbox::{SandboxManager, ShellExecutionResult};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellExecutionPlan {
    pub policy: PermissionPolicy,
    pub sandboxed_workdir: String,
    pub timeout_secs: u64,
}

pub fn plan_shell_execution(
    sandbox: &SandboxManager,
    workdir: &Path,
    timeout: Duration,
) -> AppResult<ShellExecutionPlan> {
    let manager = PermissionManager::new();
    let (policy, _, _) = manager.classify("run_shell");
    let workspace = sandbox.prepare_workspace(workdir)?;
    Ok(ShellExecutionPlan {
        policy,
        sandboxed_workdir: workspace.path.display().to_string(),
        timeout_secs: timeout.as_secs(),
    })
}

pub fn run_shell(
    sandbox: &SandboxManager,
    workdir: &Path,
    command: &str,
    timeout: Duration,
) -> AppResult<ShellExecutionResult> {
    let manager = PermissionManager::new();
    let (policy, _, _) = manager.ensure_allowed("run_shell")?;
    if policy == PermissionPolicy::AlwaysDeny {
        return Err(AppError::new("当前 Harness 已拒绝 run_shell"));
    }

    let workspace = sandbox.prepare_workspace(workdir)?;
    let sandboxed_workdir = resolve_sandboxed_workdir(&workspace.path, workdir)?;
    let start = Instant::now();
    let mut child = Command::new("cmd")
        .args(["/C", command])
        .current_dir(&sandboxed_workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    loop {
        if let Some(status) = child.try_wait()? {
            let output = child.wait_with_output()?;
            return Ok(ShellExecutionResult {
                exit_code: status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                duration_ms: start.elapsed().as_millis(),
            });
        }

        if start.elapsed() >= timeout {
            child.kill()?;
            let output = child.wait_with_output()?;
            return Ok(ShellExecutionResult {
                exit_code: -1,
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: format!(
                    "timeout after {}s\n{}",
                    timeout.as_secs(),
                    String::from_utf8_lossy(&output.stderr)
                ),
                duration_ms: start.elapsed().as_millis(),
            });
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn resolve_sandboxed_workdir(
    workspace_root: &Path,
    _source_workdir: &Path,
) -> AppResult<std::path::PathBuf> {
    if !workspace_root.exists() {
        std::fs::create_dir_all(workspace_root)?;
    }

    Ok(workspace_root.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn executes_shell_command() {
        let root = std::env::temp_dir().join(format!("codeforge-shell-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("temp dir should exist");
        std::fs::write(root.join("sample.txt"), "hello").expect("source file should exist");
        let sandbox = SandboxManager::new(root.join("sandbox")).expect("sandbox should initialize");

        let result = run_shell(&sandbox, &root, "type sample.txt", Duration::from_secs(5))
            .expect("shell should execute");
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.to_lowercase().contains("hello"));
    }

    #[test]
    fn returns_timeout_result() {
        let root =
            std::env::temp_dir().join(format!("codeforge-shell-timeout-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("temp dir should exist");
        let sandbox = SandboxManager::new(root.join("sandbox")).expect("sandbox should initialize");

        let result = run_shell(
            &sandbox,
            &root,
            "ping -n 5 127.0.0.1 >nul",
            Duration::from_millis(200),
        )
        .expect("timeout result should still return");
        assert_eq!(result.exit_code, -1);
        assert!(result.stderr.contains("timeout"));
    }
}
