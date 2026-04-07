use std::collections::HashSet;
use std::path::{Path, PathBuf};

use regex::Regex;
use tauri::{AppHandle, Emitter, State};
use walkdir::{DirEntry, WalkDir};

use crate::state::AppState;
use crate::tools::analysis_tools::find_code_smells;

const MAX_REVIEW_ISSUES: usize = 3_000;
const PROGRESS_INTERVAL: usize = 120;
const LONG_LINE_LIMIT: usize = 180;
const LARGE_FILE_LINES: usize = 1_200;

const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".idea",
    ".vscode",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    "coverage",
    "tmp",
    "temp",
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub path: String,
    pub file_count: usize,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewIssue {
    pub file: String,
    pub line: usize,
    pub rule: String,
    pub severity: String,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone)]
struct HeuristicRule {
    name: &'static str,
    severity: &'static str,
    message: &'static str,
    suggestion: &'static str,
    regex: Regex,
}

#[tauri::command]
pub fn project_open(path: String) -> Result<ProjectInfo, String> {
    let path_buf = PathBuf::from(&path);
    let file_count = WalkDir::new(&path_buf)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .count();

    Ok(ProjectInfo {
        name: path_buf
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("project")
            .to_string(),
        path,
        file_count,
    })
}

#[tauri::command]
pub fn project_clone(state: State<'_, AppState>, git_url: String) -> Result<ProjectInfo, String> {
    let target = state
        .config
        .sandbox_root
        .join(format!("repo-{}", uuid::Uuid::new_v4()));
    clone_repo(&git_url, &target).map_err(|error| error.message)?;
    let info = project_open(target.display().to_string())?;
    state
        .logs
        .record(
            "project_clone",
            serde_json::json!({ "gitUrl": git_url, "path": info.path }),
        )
        .map_err(|error| error.message)?;
    Ok(info)
}

#[tauri::command]
pub fn project_review<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    path: String,
    sandbox: bool,
) -> Result<(), String> {
    let source = PathBuf::from(&path);
    let review_root = if sandbox {
        state
            .sandbox
            .prepare_workspace(&source)
            .map_err(|error| error.message.clone())?
            .path
    } else {
        source.clone()
    };

    app.emit(
        "review_progress",
        serde_json::json!({ "step": "scan", "log": format!("准备审查目录：{}", review_root.display()) }),
    )
    .map_err(|error| error.to_string())?;

    let issues = collect_review_issues_with_progress(&app, &review_root)
        .map_err(|error| error.message.clone())?;

    app.emit(
        "review_progress",
        serde_json::json!({ "step": "complete", "log": format!("审查完成，共发现 {} 个问题", issues.len()) }),
    )
    .map_err(|error| error.to_string())?;
    app.emit("review_result", &issues)
        .map_err(|error| error.to_string())?;
    state
        .logs
        .record(
            "project_review",
            serde_json::json!({
                "path": review_root.display().to_string(),
                "issueCount": issues.len(),
                "sandbox": sandbox,
            }),
        )
        .map_err(|error| error.message)?;
    Ok(())
}

pub fn clone_repo(git_url: &str, target: &PathBuf) -> crate::error::AppResult<()> {
    let result = std::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            git_url,
            &target.display().to_string(),
        ])
        .output()?;
    if !result.status.success() {
        return Err(crate::error::AppError::new(
            String::from_utf8_lossy(&result.stderr).to_string(),
        ));
    }
    Ok(())
}

pub fn collect_review_issues(review_root: &PathBuf) -> crate::error::AppResult<Vec<ReviewIssue>> {
    collect_review_issues_internal(review_root, |_| {})
}

fn collect_review_issues_with_progress<R: tauri::Runtime>(
    app: &AppHandle<R>,
    review_root: &PathBuf,
) -> crate::error::AppResult<Vec<ReviewIssue>> {
    collect_review_issues_internal(review_root, |log| {
        let _ = app.emit(
            "review_progress",
            serde_json::json!({ "step": "scan", "log": log }),
        );
    })
}

fn collect_review_issues_internal<F>(
    review_root: &PathBuf,
    mut progress: F,
) -> crate::error::AppResult<Vec<ReviewIssue>>
where
    F: FnMut(String),
{
    let targets = collect_review_targets(review_root);
    progress(format!("识别到 {} 个候选文件。", targets.len()));

    let mut issues = collect_smell_review_issues(review_root)?;
    progress(format!("规则检查命中 {} 条问题。", issues.len()));

    let heuristic_issues =
        collect_heuristic_review_issues(review_root, &targets, |index, total| {
            if index == 0 || index % PROGRESS_INTERVAL == 0 || index == total {
                progress(format!("启发式扫描进度：{}/{}", index, total));
            }
        })?;
    issues.extend(heuristic_issues);

    let deduped = dedupe_and_sort_issues(issues);
    if deduped.is_empty() {
        return Ok(vec![ReviewIssue {
            file: "(summary)".into(),
            line: 1,
            rule: "no-obvious-issues".into(),
            severity: "info".into(),
            message: "当前规则未发现明显问题。".into(),
            suggestion: "建议结合单元测试、集成测试与人工走查继续验证。".into(),
        }]);
    }

    progress(format!("最终输出 {} 条问题。", deduped.len()));
    Ok(deduped)
}

fn collect_smell_review_issues(review_root: &PathBuf) -> crate::error::AppResult<Vec<ReviewIssue>> {
    let smells = find_code_smells(review_root)?;
    let mut issues = Vec::new();
    for smell in smells {
        if smell["rule"].as_str() == Some("truncated") {
            continue;
        }

        let file = smell["file"].as_str().unwrap_or_default().to_string();
        let line = smell["line"].as_u64().unwrap_or(1) as usize;
        let rule = smell["rule"].as_str().unwrap_or("smell").to_string();
        let severity = smell["severity"].as_str().unwrap_or("warning").to_string();
        let message = smell["message"]
            .as_str()
            .unwrap_or("检测到潜在问题")
            .to_string();
        let suggestion = smell["suggestion"]
            .as_str()
            .unwrap_or("建议补充上下文并修复该问题")
            .to_string();

        issues.push(ReviewIssue {
            file,
            line,
            rule,
            severity,
            message,
            suggestion,
        });
        if issues.len() >= MAX_REVIEW_ISSUES {
            break;
        }
    }
    Ok(issues)
}

fn collect_heuristic_review_issues<F>(
    review_root: &PathBuf,
    targets: &[PathBuf],
    mut progress: F,
) -> crate::error::AppResult<Vec<ReviewIssue>>
where
    F: FnMut(usize, usize),
{
    let rules = heuristic_rules()?;
    let mut issues = Vec::new();

    for (index, file_path) in targets.iter().enumerate() {
        let completed = index + 1;
        progress(completed, targets.len());

        let content = match std::fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        let relative_path = to_relative_path(review_root, file_path);
        let line_count = content.lines().count();

        if line_count > LARGE_FILE_LINES {
            issues.push(ReviewIssue {
                file: relative_path.clone(),
                line: 1,
                rule: "large-file".into(),
                severity: "info".into(),
                message: format!("文件行数为 {}，维护成本较高。", line_count),
                suggestion: "建议拆分模块并提取可复用逻辑。".into(),
            });
        }

        let mut long_line_hits = 0usize;
        for (line_index, line) in content.lines().enumerate() {
            let line_number = line_index + 1;

            if line.chars().count() > LONG_LINE_LIMIT && long_line_hits < 2 {
                long_line_hits += 1;
                issues.push(ReviewIssue {
                    file: relative_path.clone(),
                    line: line_number,
                    rule: "long-line".into(),
                    severity: "info".into(),
                    message: format!("单行长度超过 {} 个字符。", LONG_LINE_LIMIT),
                    suggestion: "建议拆分表达式并增加可读性。".into(),
                });
            }

            for rule in &rules {
                if rule.regex.is_match(line) {
                    issues.push(ReviewIssue {
                        file: relative_path.clone(),
                        line: line_number,
                        rule: rule.name.to_string(),
                        severity: rule.severity.to_string(),
                        message: rule.message.to_string(),
                        suggestion: rule.suggestion.to_string(),
                    });
                }
            }

            if issues.len() >= MAX_REVIEW_ISSUES {
                return Ok(issues);
            }
        }

        if issues.len() >= MAX_REVIEW_ISSUES {
            break;
        }
    }

    Ok(issues)
}

fn heuristic_rules() -> crate::error::AppResult<Vec<HeuristicRule>> {
    Ok(vec![
        HeuristicRule {
            name: "ts-ignore",
            severity: "warning",
            message: "检测到 TypeScript 类型抑制注释。",
            suggestion: "建议修复类型定义，减少抑制指令。",
            regex: Regex::new(r"@ts-ignore|@ts-expect-error")
                .map_err(|error| crate::error::AppError::new(error.to_string()))?,
        },
        HeuristicRule {
            name: "any-cast",
            severity: "warning",
            message: "检测到 as any，可能绕过类型约束。",
            suggestion: "建议补齐具体类型，避免降级为 any。",
            regex: Regex::new(r"\bas\s+any\b")
                .map_err(|error| crate::error::AppError::new(error.to_string()))?,
        },
        HeuristicRule {
            name: "console-log",
            severity: "info",
            message: "检测到 console 输出。",
            suggestion: "建议替换为统一日志组件，避免噪声输出。",
            regex: Regex::new(r"\bconsole\.(log|debug|info)\s*\(")
                .map_err(|error| crate::error::AppError::new(error.to_string()))?,
        },
        HeuristicRule {
            name: "debugger",
            severity: "warning",
            message: "检测到 debugger 语句。",
            suggestion: "建议在提交前移除调试断点。",
            regex: Regex::new(r"\bdebugger\b")
                .map_err(|error| crate::error::AppError::new(error.to_string()))?,
        },
        HeuristicRule {
            name: "hardcoded-secret",
            severity: "error",
            message: "检测到疑似明文密钥或口令。",
            suggestion: "建议改为环境变量或密钥管理服务。",
            regex: Regex::new(
                r#"(?i)\b(api[_-]?key|access[_-]?token|secret|password)\b.{0,24}[:=].{0,4}["'][^"'\s]{12,}["']"#,
            )
            .map_err(|error| crate::error::AppError::new(error.to_string()))?,
        },
        HeuristicRule {
            name: "empty-catch",
            severity: "warning",
            message: "检测到空 catch 代码块。",
            suggestion: "建议记录错误上下文并明确恢复策略。",
            regex: Regex::new(r"catch\s*\([^\)]*\)\s*\{\s*\}")
                .map_err(|error| crate::error::AppError::new(error.to_string()))?,
        },
        HeuristicRule {
            name: "python-bare-except",
            severity: "warning",
            message: "检测到裸 except。",
            suggestion: "建议指定异常类型并记录错误上下文。",
            regex: Regex::new(r"^\s*except\s*:\s*$")
                .map_err(|error| crate::error::AppError::new(error.to_string()))?,
        },
        HeuristicRule {
            name: "eval-usage",
            severity: "error",
            message: "检测到动态执行语句，存在安全风险。",
            suggestion: "建议使用白名单解析或受控执行方案。",
            regex: Regex::new(r"\b(eval|new\s+Function)\s*\(")
                .map_err(|error| crate::error::AppError::new(error.to_string()))?,
        },
    ])
}

fn collect_review_targets(review_root: &Path) -> Vec<PathBuf> {
    WalkDir::new(review_root)
        .into_iter()
        .filter_entry(should_walk_dir)
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && should_review_file(entry.path()))
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>()
}

fn should_walk_dir(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let dir = entry.file_name().to_string_lossy();
    !IGNORED_DIRS.iter().any(|item| *item == dir)
}

fn should_review_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    matches!(
        ext.as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "go"
            | "java"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "cs"
            | "json"
            | "yaml"
            | "yml"
            | "toml"
            | "md"
    )
}

fn to_relative_path(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

fn dedupe_and_sort_issues(issues: Vec<ReviewIssue>) -> Vec<ReviewIssue> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    for issue in issues {
        let key = format!(
            "{}:{}:{}:{}",
            issue.file, issue.line, issue.rule, issue.message
        );
        if seen.insert(key) {
            unique.push(issue);
        }
        if unique.len() >= MAX_REVIEW_ISSUES {
            break;
        }
    }

    unique.sort_by(|left, right| {
        let severity_cmp = severity_rank(&left.severity).cmp(&severity_rank(&right.severity));
        if severity_cmp != std::cmp::Ordering::Equal {
            return severity_cmp;
        }
        let file_cmp = left.file.cmp(&right.file);
        if file_cmp != std::cmp::Ordering::Equal {
            return file_cmp;
        }
        left.line.cmp(&right.line)
    });
    unique
}

fn severity_rank(severity: &str) -> usize {
    match severity {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviews_local_codeforge_repo() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri parent should exist")
            .to_path_buf();
        let issues = collect_review_issues(&repo_root).expect("local repo review should succeed");
        assert!(!issues.is_empty());
    }

    #[test]
    #[ignore]
    fn clones_and_reviews_remote_repo() {
        let target =
            std::env::temp_dir().join(format!("codeforge-remote-review-{}", uuid::Uuid::new_v4()));
        clone_repo("https://github.com/Mag1cFall/CodeForge", &target)
            .expect("remote repo clone should succeed");
        let issues = collect_review_issues(&target).expect("remote repo review should succeed");
        assert!(!issues.is_empty());
    }
}
