use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};
use crate::harness::sandbox::SandboxManager;

use super::analysis_tools::{analyze_ast, check_complexity, find_code_smells, suggest_refactor};
use super::emit_structured_log;
use super::file_tools::{
    apply_patch_text, apply_structured_patch, list_directory, read_file, resolve_path, write_file,
};
use super::schema::ToolSchema;
use super::search_tools::{grep_pattern, search_code};
use super::shell_tools::run_shell;

#[derive(Debug, Clone)]
pub struct ToolRegistry {
    sandbox_manager: SandboxManager,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    pub workspace_root: Option<PathBuf>,
}

impl ToolRegistry {
    pub fn new(sandbox_manager: SandboxManager) -> Self {
        Self { sandbox_manager }
    }

    pub fn list(&self) -> Vec<ToolSchema> {
        vec![
            ToolSchema {
                name: "read_file".into(),
                description: "读取文件并附带 Hashline".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }),
            },
            ToolSchema {
                name: "list_directory".into(),
                description: "列出目录内容".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }),
            },
            ToolSchema {
                name: "search_code".into(),
                description: "按字符串搜索代码".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "query": { "type": "string" }, "path": { "type": "string" } }, "required": ["query", "path"] }),
            },
            ToolSchema {
                name: "grep_pattern".into(),
                description: "按正则搜索代码".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "pattern": { "type": "string" }, "path": { "type": "string" } }, "required": ["pattern", "path"] }),
            },
            ToolSchema {
                name: "run_shell".into(),
                description: "在工作区内执行 Shell 命令".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "command": { "type": "string" }, "workdir": { "type": "string" }, "timeoutSecs": { "type": "integer" } }, "required": ["command"] }),
            },
            ToolSchema {
                name: "run_tests".into(),
                description: "在工作区内执行测试命令".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "command": { "type": "string" }, "workdir": { "type": "string" }, "timeoutSecs": { "type": "integer" } }, "required": ["command"] }),
            },
            ToolSchema {
                name: "write_file".into(),
                description: "写入完整文件内容".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "content": { "type": "string" } }, "required": ["path", "content"] }),
            },
            ToolSchema {
                name: "apply_patch".into(),
                description: "使用标准补丁格式批量修改文件（兼容旧版 path/old/new 参数）".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "input": { "type": "string" }, "path": { "type": "string" }, "old": { "type": "string" }, "new": { "type": "string" } }, "required": ["input"] }),
            },
            ToolSchema {
                name: "analyze_ast".into(),
                description: "基于文件文本做结构统计".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }),
            },
            ToolSchema {
                name: "check_complexity".into(),
                description: "估算文件的圈复杂度".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }),
            },
            ToolSchema {
                name: "find_code_smells".into(),
                description: "扫描常见代码异味".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }),
            },
            ToolSchema {
                name: "suggest_refactor".into(),
                description: "根据异味生成重构建议".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }),
            },
        ]
    }

    pub fn execute(
        &self,
        name: &str,
        args: serde_json::Value,
        context: &ToolExecutionContext,
    ) -> AppResult<String> {
        let started = Instant::now();
        emit_structured_log(
            "registry",
            "tool_execute_start",
            serde_json::json!({
                "name": name,
                "workspaceRoot": context.workspace_root.as_ref().map(|path| path.display().to_string()),
                "args": summarize_args(&args),
            }),
        );

        let result = match name {
            "read_file" => {
                let path = require_string(&args, "path")?;
                let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                read_file(resolved)
            }
            "list_directory" => {
                let path = require_string(&args, "path")?;
                let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                Ok(list_directory(resolved)?.join("\n"))
            }
            "search_code" => {
                let path = require_string(&args, "path")?;
                let query = require_string(&args, "query")?;
                let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                Ok(search_code(&resolved, query)?.join("\n"))
            }
            "grep_pattern" => {
                let path = require_string(&args, "path")?;
                let pattern = require_string(&args, "pattern")?;
                let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                Ok(grep_pattern(&resolved, pattern)?.join("\n"))
            }
            "run_shell" | "run_tests" => {
                let command = require_string(&args, "command")?;
                let workdir = args
                    .get("workdir")
                    .and_then(|value| value.as_str())
                    .map(|value| resolve_workdir(context.workspace_root.as_deref(), value))
                    .transpose()?
                    .or_else(|| context.workspace_root.clone())
                    .ok_or_else(|| AppError::new("run_shell 缺少工作目录"))?;
                let timeout = args
                    .get("timeoutSecs")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(30);
                let result = run_shell(
                    &self.sandbox_manager,
                    &workdir,
                    command,
                    Duration::from_secs(timeout),
                )?;
                Ok(serde_json::to_string(&result)?)
            }
            "write_file" => {
                let path = require_string(&args, "path")?;
                let content = require_string(&args, "content")?;
                let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                write_file(&resolved, content)?;
                Ok(format!("WROTE {}", resolved.display()))
            }
            "apply_patch" => {
                if let Some(input) = args
                    .get("input")
                    .and_then(|value| value.as_str())
                    .filter(|value| !value.trim().is_empty())
                {
                    let workspace_root = context
                        .workspace_root
                        .as_deref()
                        .ok_or_else(|| AppError::new("apply_patch 缺少工作区根目录"))?;
                    apply_structured_patch(workspace_root, input)
                } else {
                    let path = require_string(&args, "path")?;
                    let old = require_string(&args, "old")?;
                    let new = require_string(&args, "new")?;
                    let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                    apply_patch_text(&resolved, old, new)
                }
            }
            "analyze_ast" => {
                let path = require_string(&args, "path")?;
                let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                Ok(serde_json::to_string(&analyze_ast(&resolved)?)?)
            }
            "check_complexity" => {
                let path = require_string(&args, "path")?;
                let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                Ok(serde_json::json!({ "complexity": check_complexity(&resolved)? }).to_string())
            }
            "find_code_smells" => {
                let path = require_string(&args, "path")?;
                let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                Ok(serde_json::to_string(&find_code_smells(&resolved)?)?)
            }
            "suggest_refactor" => {
                let path = require_string(&args, "path")?;
                let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                Ok(serde_json::to_string(&suggest_refactor(&resolved)?)?)
            }
            _ => Err(AppError::new(format!("未知工具: {name}"))),
        };

        match &result {
            Ok(output) => {
                emit_structured_log(
                    "registry",
                    "tool_execute_finish",
                    serde_json::json!({
                        "name": name,
                        "status": "ok",
                        "elapsedMs": started.elapsed().as_millis(),
                        "resultBytes": output.len(),
                    }),
                );
            }
            Err(error) => {
                emit_structured_log(
                    "registry",
                    "tool_execute_finish",
                    serde_json::json!({
                        "name": name,
                        "status": "error",
                        "elapsedMs": started.elapsed().as_millis(),
                        "error": error.message.clone(),
                    }),
                );
            }
        }

        result
    }
}

fn resolve_workdir(root: Option<&Path>, input: &str) -> AppResult<PathBuf> {
    let trimmed = input.trim();
    if let Some(root) = root {
        if trimmed.is_empty() || trimmed == "." || trimmed == "/" || trimmed == "\\" {
            return Ok(root.to_path_buf());
        }

        let path = PathBuf::from(trimmed);
        if path.is_absolute() {
            if path.starts_with(root) {
                return Ok(if path.exists() {
                    path
                } else {
                    root.to_path_buf()
                });
            }
            return Ok(root.to_path_buf());
        }

        let candidate = root.join(path);
        return Ok(if candidate.exists() {
            candidate
        } else {
            root.to_path_buf()
        });
    }

    resolve_path(None, trimmed)
}

fn require_string<'a>(args: &'a serde_json::Value, key: &str) -> AppResult<&'a str> {
    args.get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new(format!("缺少参数: {key}")))
}

fn summarize_args(args: &serde_json::Value) -> serde_json::Value {
    truncate_json(args, 0)
}

fn truncate_json(value: &serde_json::Value, depth: usize) -> serde_json::Value {
    if depth >= 3 {
        return serde_json::json!("<truncated>");
    }

    match value {
        serde_json::Value::String(text) => {
            if text.chars().count() <= 160 {
                serde_json::json!(text)
            } else {
                serde_json::json!(format!("{}...", text.chars().take(160).collect::<String>()))
            }
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .take(20)
                .map(|item| truncate_json(item, depth + 1))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .take(20)
                .map(|(key, value)| (key.clone(), truncate_json(value, depth + 1)))
                .collect(),
        ),
        other => other.clone(),
    }
}
