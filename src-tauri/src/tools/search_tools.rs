use std::path::Path;

use regex::Regex;
use walkdir::{DirEntry, WalkDir};

use crate::error::{AppError, AppResult};

use super::emit_structured_log;

const MAX_RESULTS: usize = 5_000;
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

pub fn search_code(root: &Path, query: &str) -> AppResult<Vec<String>> {
    if query.trim().is_empty() {
        return Err(AppError::new("搜索内容不能为空"));
    }
    if !root.exists() {
        return Err(AppError::new(format!("路径不存在: {}", root.display())));
    }

    let mut results = Vec::new();
    let mut scanned_files = 0usize;
    let mut truncated = false;

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

        for (index, line) in content.lines().enumerate() {
            if line.contains(query) {
                results.push(format_match(entry.path(), index + 1, line));
                if results.len() >= MAX_RESULTS {
                    truncated = true;
                    break 'entries;
                }
            }
        }
    }

    if truncated {
        results.push(format!("[结果已截断，仅返回前 {MAX_RESULTS} 条匹配]"));
    }

    emit_structured_log(
        "search_tools",
        "search_code",
        serde_json::json!({
            "root": root.display().to_string(),
            "query": query,
            "scannedFiles": scanned_files,
            "resultCount": results.len(),
            "truncated": truncated,
        }),
    );

    Ok(results)
}

pub fn grep_pattern(root: &Path, pattern: &str) -> AppResult<Vec<String>> {
    if pattern.trim().is_empty() {
        return Err(AppError::new("正则表达式不能为空"));
    }
    if !root.exists() {
        return Err(AppError::new(format!("路径不存在: {}", root.display())));
    }

    let regex = Regex::new(pattern).map_err(|error| AppError::new(error.to_string()))?;
    let mut results = Vec::new();
    let mut scanned_files = 0usize;
    let mut truncated = false;

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

        for (index, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                results.push(format_match(entry.path(), index + 1, line));
                if results.len() >= MAX_RESULTS {
                    truncated = true;
                    break 'entries;
                }
            }
        }
    }

    if truncated {
        results.push(format!("[结果已截断，仅返回前 {MAX_RESULTS} 条匹配]"));
    }

    emit_structured_log(
        "search_tools",
        "grep_pattern",
        serde_json::json!({
            "root": root.display().to_string(),
            "pattern": pattern,
            "scannedFiles": scanned_files,
            "resultCount": results.len(),
            "truncated": truncated,
        }),
    );

    Ok(results)
}

fn should_walk_entry(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !IGNORED_DIRS.iter().any(|segment| *segment == name)
}

fn format_match(path: &Path, line: usize, content: &str) -> String {
    format!("{}:{}: {}", path.display(), line, content.trim())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_matches_in_workspace() {
        let dir = std::env::temp_dir().join(format!("codeforge-search-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should exist");
        std::fs::write(dir.join("a.rs"), "fn demo() { println!(\"hello\"); }")
            .expect("file should exist");

        let results = search_code(&dir, "demo").expect("search should work");
        assert_eq!(results.len(), 1);
        let grep_results = grep_pattern(&dir, "println").expect("grep should work");
        assert_eq!(grep_results.len(), 1);
    }

    #[test]
    fn skips_ignored_directories() {
        let dir =
            std::env::temp_dir().join(format!("codeforge-search-skip-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("node_modules")).expect("temp dir should exist");
        std::fs::write(dir.join("node_modules").join("skip.ts"), "const mark = 1;")
            .expect("test file should exist");

        let results = search_code(&dir, "mark").expect("search should work");
        assert!(results.is_empty());
    }
}
