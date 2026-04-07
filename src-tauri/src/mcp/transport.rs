use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::error::{AppError, AppResult};

use super::log_structured;

pub fn exchange_stdio(
    command: &str,
    args: &[String],
    env: &BTreeMap<String, String>,
    requests: &[serde_json::Value],
) -> AppResult<Vec<serde_json::Value>> {
    log_structured(
        "mcp.transport",
        "stdio.spawn",
        serde_json::json!({
            "command": command,
            "args": args,
            "requestCount": requests.len(),
            "envKeyCount": env.len(),
        }),
    );

    let mut child = Command::new(command);
    child
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(merge_process_env(env));
    let mut child = child.spawn()?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::new("MCP 子进程 stdin 不可用"))?;
    for request in requests {
        writeln!(stdin, "{}", serde_json::to_string(request)?)?;
    }
    drop(stdin);

    let output = child.wait_with_output()?;

    let mut responses = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(line.trim()) {
            Ok(value) => responses.push(value),
            Err(error) => {
                log_structured(
                    "mcp.transport",
                    "stdio.stdout.non_json_line",
                    serde_json::json!({
                        "error": error.to_string(),
                        "line": compact_message(line),
                    }),
                );
            }
        }
    }

    let status = output.status;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    log_structured(
        "mcp.transport",
        "stdio.exit",
        serde_json::json!({
            "status": status.to_string(),
            "success": status.success(),
            "responseCount": responses.len(),
            "stderr": if stderr.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(compact_message(&stderr)) },
        }),
    );

    if !status.success() && responses.is_empty() {
        let detail = if stderr.is_empty() {
            format!("MCP 子进程退出状态异常: {status}")
        } else {
            format!("MCP 子进程退出状态异常: {status}; stderr: {stderr}")
        };
        return Err(AppError::new(detail));
    }
    Ok(responses)
}

fn compact_message(message: &str) -> String {
    const LIMIT: usize = 400;
    let trimmed = message.trim();
    if trimmed.chars().count() <= LIMIT {
        return trimmed.to_string();
    }
    let preview = trimmed.chars().take(LIMIT).collect::<String>();
    format!("{preview}…")
}

fn merge_process_env(extra_env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut merged = extra_env.clone();

    if cfg!(windows) {
        let existing = merged
            .keys()
            .map(|key| key.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        for (key, value) in std::env::vars() {
            if !existing.contains(&key.to_ascii_lowercase()) {
                merged.insert(key, value);
            }
        }
        return merged;
    }

    for (key, value) in std::env::vars() {
        merged.entry(key).or_insert(value);
    }
    merged
}
