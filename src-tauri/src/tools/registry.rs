use std::path::PathBuf;
use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::harness::sandbox::SandboxManager;

use super::analysis_tools::{analyze_ast, check_complexity, find_code_smells, suggest_refactor};
use super::file_tools::{apply_patch_text, list_directory, read_file, resolve_path, write_file};
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
                description: "基于原始片段进行单次文本补丁替换".into(),
                parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "old": { "type": "string" }, "new": { "type": "string" } }, "required": ["path", "old", "new"] }),
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
        match name {
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
                    .map(|value| resolve_path(context.workspace_root.as_deref(), value))
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
                let path = require_string(&args, "path")?;
                let old = require_string(&args, "old")?;
                let new = require_string(&args, "new")?;
                let resolved = resolve_path(context.workspace_root.as_deref(), path)?;
                let updated = apply_patch_text(&resolved, old, new)?;
                Ok(updated)
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
        }
    }
}

fn require_string<'a>(args: &'a serde_json::Value, key: &str) -> AppResult<&'a str> {
    args.get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new(format!("缺少参数: {key}")))
}
