use std::path::Path;
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};
use crate::harness::permission::{PermissionManager, PermissionPolicy};
use crate::harness::sandbox::{SandboxManager, ShellExecutionResult};

use super::emit_structured_log;

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
    let plan = ShellExecutionPlan {
        policy,
        sandboxed_workdir: workspace.path.display().to_string(),
        timeout_secs: timeout.as_secs(),
    };
    emit_structured_log(
        "shell_tools",
        "plan_shell_execution",
        serde_json::json!({
            "workdir": workdir.display().to_string(),
            "sandboxedWorkdir": plan.sandboxed_workdir,
            "timeoutSecs": plan.timeout_secs,
            "policy": plan.policy,
        }),
    );
    Ok(plan)
}

pub fn run_shell(
    sandbox: &SandboxManager,
    workdir: &Path,
    command: &str,
    timeout: Duration,
) -> AppResult<ShellExecutionResult> {
    let command = command.trim();
    if command.is_empty() {
        return Err(AppError::new("run_shell 缺少 command 参数"));
    }

    let manager = PermissionManager::new();
    let (policy, _, _) = manager.ensure_allowed("run_shell")?;
    if policy == PermissionPolicy::AlwaysDeny {
        return Err(AppError::new("当前 Harness 已拒绝 run_shell"));
    }

    let normalized_command = normalize_shell_command(command);
    let started = Instant::now();

    emit_structured_log(
        "shell_tools",
        "run_shell_start",
        serde_json::json!({
            "workdir": workdir.display().to_string(),
            "timeoutSecs": timeout.as_secs(),
            "commandPreview": preview_command(&normalized_command),
        }),
    );

    let (mut result, workspace) =
        sandbox.run_command_with_workspace(&normalized_command, workdir, timeout)?;
    sanitize_shell_output(&mut result, &workspace.path, workdir);
    result.duration_ms = started.elapsed().as_millis();

    emit_structured_log(
        "shell_tools",
        "run_shell_finish",
        serde_json::json!({
            "workdir": workdir.display().to_string(),
            "exitCode": result.exit_code,
            "durationMs": result.duration_ms,
            "stdoutBytes": result.stdout.len(),
            "stderrBytes": result.stderr.len(),
        }),
    );

    Ok(result)
}

fn sanitize_shell_output(
    result: &mut ShellExecutionResult,
    sandboxed_workdir: &Path,
    workdir: &Path,
) {
    let sandbox_path = sandboxed_workdir.display().to_string();
    let source_path = workdir.display().to_string();
    if sandbox_path.is_empty() || source_path.is_empty() {
        return;
    }

    result.stdout = result.stdout.replace(&sandbox_path, &source_path);
    result.stderr = result.stderr.replace(&sandbox_path, &source_path);
}

fn normalize_shell_command(command: &str) -> String {
    let mut normalized = command.to_string();
    if cfg!(windows) {
        normalized = normalized.replace("pwd &&", "cd &&");
        normalized = normalized.replace("&& pwd", "&& cd");
        normalized = normalized.replace("; pwd", "& cd");
        normalized = normalized.replace("pwd;", "cd&");
        if normalized.trim() == "pwd" {
            normalized = "cd".into();
        }
        normalized = normalized.replace(" ls ", " dir ");
        normalized = normalized.replace("&& ls", "&& dir");
        normalized = normalized.replace("; ls", "& dir");
        if normalized.trim().starts_with("ls ") {
            normalized = normalized.replacen("ls ", "dir ", 1);
        }
        if normalized.trim() == "ls" {
            normalized = "dir".into();
        }

        normalized = normalized.replace("dir -la", "dir");
        normalized = normalized.replace("dir -al", "dir");
        normalized = normalized.replace("dir -l", "dir");
        normalized = normalized.replace("dir -a", "dir");
    }
    normalized
}

fn preview_command(command: &str) -> String {
    let compact = command.replace('\n', "\\n");
    if compact.chars().count() <= 200 {
        compact
    } else {
        format!("{}...", compact.chars().take(200).collect::<String>())
    }
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

    #[test]
    fn normalizes_common_unix_aliases_for_windows_shell() {
        let normalized = normalize_shell_command("pwd && ls");
        assert!(normalized.contains("cd"));
        assert!(normalized.contains("dir"));
    }

    #[test]
    fn maps_sandbox_workdir_back_to_source_path() {
        let root = std::env::temp_dir().join(format!(
            "codeforge-shell-workdir-map-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("temp dir should exist");
        let sandbox = SandboxManager::new(root.join("sandbox")).expect("sandbox should initialize");

        let result = run_shell(&sandbox, &root, "pwd", Duration::from_secs(5))
            .expect("shell should execute");
        let output = result.stdout.to_lowercase();

        assert!(output.contains(&root.display().to_string().to_lowercase()));
        assert!(!output.contains("workspace-"));
    }
}
