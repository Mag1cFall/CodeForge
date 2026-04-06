use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use crate::error::{AppError, AppResult};

pub fn exchange_stdio(
    command: &str,
    args: &[String],
    requests: &[serde_json::Value],
) -> AppResult<Vec<serde_json::Value>> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::new("MCP 子进程 stdin 不可用"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::new("MCP 子进程 stdout 不可用"))?;
    let mut reader = BufReader::new(stdout);

    for request in requests {
        writeln!(stdin, "{}", serde_json::to_string(request)?)?;
    }
    drop(stdin);

    let mut responses = Vec::new();
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str(line.trim()) {
            responses.push(value);
        }
    }

    let status = child.wait()?;
    if !status.success() && responses.is_empty() {
        return Err(AppError::new(format!("MCP 子进程退出状态异常: {status}")));
    }
    Ok(responses)
}
