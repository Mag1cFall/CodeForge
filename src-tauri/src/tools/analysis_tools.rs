use std::path::Path;

use regex::Regex;
use walkdir::{DirEntry, WalkDir};

use crate::error::{AppError, AppResult};

use super::emit_structured_log;

const MAX_SMELL_RESULTS: usize = 5_000;
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

pub fn analyze_ast(path: &Path) -> AppResult<serde_json::Value> {
    let content = read_text_file(path)?;
    let language = detect_language(path);
    let line_count = content.lines().count();
    let blank_line_count = content
        .lines()
        .filter(|line| line.trim().is_empty())
        .count();
    let comment_line_count = count_comment_lines(&content, language);
    let function_count = count_functions(&content, language)?;
    let complexity = estimate_complexity_score(&content)?;

    let report = serde_json::json!({
        "file": path.display().to_string(),
        "language": language,
        "lineCount": line_count,
        "blankLineCount": blank_line_count,
        "commentLineCount": comment_line_count,
        "functionCount": function_count,
        "complexity": complexity,
    });
    emit_structured_log("analysis_tools", "analyze_ast", report.clone());
    Ok(report)
}

pub fn check_complexity(path: &Path) -> AppResult<usize> {
    let content = read_text_file(path)?;
    let complexity = estimate_complexity_score(&content)?;
    emit_structured_log(
        "analysis_tools",
        "check_complexity",
        serde_json::json!({
            "path": path.display().to_string(),
            "complexity": complexity,
        }),
    );
    Ok(complexity)
}

pub fn find_code_smells(root: &Path) -> AppResult<Vec<serde_json::Value>> {
    if !root.exists() {
        return Err(AppError::new(format!("路径不存在: {}", root.display())));
    }

    let rules = smell_rules();
    let mut results = Vec::new();
    let mut scanned_files = 0usize;

    'entries: for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(should_walk_entry)
    {
        let entry = entry.map_err(|error| AppError::new(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        scanned_files += 1;
        let Ok(content) = read_text_file(entry.path()) else {
            continue;
        };

        for (line_number, line) in content.lines().enumerate() {
            for rule in &rules {
                if rule.regex.is_match(line) {
                    let relative_path = entry.path()
                        .strip_prefix(root)
                        .unwrap_or(entry.path())
                        .to_string_lossy()
                        .replace('\\', "/");
                        
                    results.push(serde_json::json!({
                        "file": relative_path,
                        "line": line_number + 1,
                        "rule": rule.name,
                        "severity": rule.severity,
                        "message": rule.message,
                        "suggestion": rule.suggestion,
                    }));
                    if results.len() >= MAX_SMELL_RESULTS {
                        results.push(serde_json::json!({
                            "rule": "truncated",
                            "message": format!("结果已截断，仅返回前 {MAX_SMELL_RESULTS} 条"),
                        }));
                        break 'entries;
                    }
                }
            }
        }
    }

    emit_structured_log(
        "analysis_tools",
        "find_code_smells",
        serde_json::json!({
            "root": root.display().to_string(),
            "scannedFiles": scanned_files,
            "resultCount": results.len(),
        }),
    );
    Ok(results)
}

pub fn suggest_refactor(root: &Path) -> AppResult<Vec<String>> {
    let smells = find_code_smells(root)?;
    let mut suggestions = Vec::new();
    for item in smells {
        let file = item
            .get("file")
            .and_then(|value| value.as_str())
            .unwrap_or("<unknown>");
        let line = item
            .get("line")
            .and_then(|value| value.as_u64())
            .unwrap_or_default();
        let suggestion = item
            .get("suggestion")
            .and_then(|value| value.as_str())
            .unwrap_or("请检查实现逻辑");
        suggestions.push(format!("{file}:{line} -> {suggestion}"));
    }

    emit_structured_log(
        "analysis_tools",
        "suggest_refactor",
        serde_json::json!({
            "root": root.display().to_string(),
            "resultCount": suggestions.len(),
        }),
    );
    Ok(suggestions)
}

fn estimate_complexity_score(content: &str) -> AppResult<usize> {
    let mut score = 1usize;
    for pattern in [
        r"\bif\b",
        r"\belse\s+if\b",
        r"\bmatch\b",
        r"\bfor\b",
        r"\bwhile\b",
        r"\bcatch\b",
        r"\?\s*[^:]*:",
        r"&&",
        r"\|\|",
    ] {
        let regex = Regex::new(pattern).map_err(|error| AppError::new(error.to_string()))?;
        score += regex.find_iter(content).count();
    }
    Ok(score)
}

fn count_functions(content: &str, language: &str) -> AppResult<usize> {
    let patterns = match language {
        "rust" => vec![r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*\s*\("],
        "python" => vec![r"(?m)^\s*def\s+[A-Za-z_][A-Za-z0-9_]*\s*\("],
        "go" => vec![r"(?m)^\s*func\s+(?:\([^\)]*\)\s*)?[A-Za-z_][A-Za-z0-9_]*\s*\("],
        "javascript" | "typescript" | "tsx" | "jsx" => vec![
            r"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+[A-Za-z_$][A-Za-z0-9_$]*\s*\(",
            r"(?m)^\s*(?:const|let|var)\s+[A-Za-z_$][A-Za-z0-9_$]*\s*=\s*(?:async\s*)?\([^\)]*\)\s*=>",
            r"(?m)^\s*[A-Za-z_$][A-Za-z0-9_$]*\s*:\s*(?:async\s*)?\([^\)]*\)\s*=>",
        ],
        _ => vec![
            r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*\s*\(",
            r"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+[A-Za-z_$][A-Za-z0-9_$]*\s*\(",
            r"(?m)^\s*def\s+[A-Za-z_][A-Za-z0-9_]*\s*\(",
            r"(?m)^\s*func\s+(?:\([^\)]*\)\s*)?[A-Za-z_][A-Za-z0-9_]*\s*\(",
        ],
    };

    let mut count = 0usize;
    for pattern in patterns {
        let regex = Regex::new(pattern).map_err(|error| AppError::new(error.to_string()))?;
        count += regex.find_iter(content).count();
    }
    Ok(count)
}

fn count_comment_lines(content: &str, language: &str) -> usize {
    match language {
        "python" => content
            .lines()
            .filter(|line| line.trim_start().starts_with('#'))
            .count(),
        _ => content
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
            })
            .count(),
    }
}

fn detect_language(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "rust",
        "py" => "python",
        "go" => "go",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "java" => "java",
        "cs" => "csharp",
        "cpp" | "cc" | "cxx" | "hpp" | "h" => "cpp",
        _ => "unknown",
    }
}

fn should_walk_entry(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !IGNORED_DIRS.iter().any(|segment| *segment == name)
}

fn read_text_file(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    if is_likely_binary(&bytes) {
        return Err(AppError::new(format!("binary file: {}", path.display())));
    }
    String::from_utf8(bytes)
        .map_err(|_| AppError::new(format!("non utf8 file: {}", path.display())))
}

fn is_likely_binary(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if bytes.contains(&0) {
        return true;
    }

    let sample = &bytes[..bytes.len().min(1024)];
    let non_text = sample
        .iter()
        .filter(|byte| !matches!(byte, b'\n' | b'\r' | b'\t' | 0x20..=0x7E))
        .count();
    non_text * 10 > sample.len() * 3
}

struct SmellRule {
    name: &'static str,
    severity: &'static str,
    message: &'static str,
    suggestion: &'static str,
    regex: Regex,
}

fn smell_rules() -> Vec<SmellRule> {
    vec![
        SmellRule {
            name: "unwrap",
            severity: "medium",
            message: "检测到 unwrap，可能在运行时触发崩溃",
            suggestion: "改用显式错误处理并返回上下文信息",
            regex: Regex::new(r"\bunwrap\s*\(").expect("unwrap regex should compile"),
        },
        SmellRule {
            name: "todo",
            severity: "high",
            message: "检测到 todo 占位实现",
            suggestion: "补齐真实逻辑，避免运行时中断",
            regex: Regex::new(r"\btodo!\s*\(").expect("todo regex should compile"),
        },
        SmellRule {
            name: "unimplemented",
            severity: "high",
            message: "检测到 unimplemented 占位实现",
            suggestion: "补齐真实逻辑，避免运行时中断",
            regex: Regex::new(r"\bunimplemented!\s*\(")
                .expect("unimplemented regex should compile"),
        },
        SmellRule {
            name: "panic",
            severity: "high",
            message: "检测到 panic，可能导致进程退出",
            suggestion: "使用可恢复错误并返回 Result",
            regex: Regex::new(r"\bpanic!\s*\(").expect("panic regex should compile"),
        },
        SmellRule {
            name: "expect",
            severity: "medium",
            message: "检测到 expect，错误路径不可恢复",
            suggestion: "替换为带上下文的错误返回",
            regex: Regex::new(r"\bexpect\s*\(").expect("expect regex should compile"),
        },
        SmellRule {
            name: "fixme",
            severity: "low",
            message: "检测到 FIXME 注释",
            suggestion: "在本次迭代中明确修复计划或移除该注释",
            regex: Regex::new(r"\bFIXME\b").expect("fixme regex should compile"),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_basic_code_smells() {
        let dir = std::env::temp_dir().join(format!("codeforge-analysis-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should exist");
        let file = dir.join("demo.rs");
        std::fs::write(&file, "fn demo() { let _ = Some(1).unwrap(); }")
            .expect("demo file should exist");

        let ast = analyze_ast(&file).expect("ast analysis should succeed");
        assert_eq!(ast["functionCount"], 1);
        assert!(check_complexity(&file).expect("complexity should be computed") >= 1);
        let smells = find_code_smells(&dir).expect("smells should be found");
        assert_eq!(
            smells
                .iter()
                .filter(|item| item["rule"] != "truncated")
                .count(),
            1
        );
    }
}
