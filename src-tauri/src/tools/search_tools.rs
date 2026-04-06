use std::path::Path;

use regex::Regex;
use walkdir::WalkDir;

use crate::error::{AppError, AppResult};

pub fn search_code(root: &Path, query: &str) -> AppResult<Vec<String>> {
    if query.trim().is_empty() {
        return Err(AppError::new("搜索内容不能为空"));
    }

    let mut results = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|error| AppError::new(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };

        for (index, line) in content.lines().enumerate() {
            if line.contains(query) {
                results.push(format!(
                    "{}:{}: {}",
                    entry.path().display(),
                    index + 1,
                    line.trim()
                ));
            }
        }
    }
    Ok(results)
}

pub fn grep_pattern(root: &Path, pattern: &str) -> AppResult<Vec<String>> {
    let regex = Regex::new(pattern).map_err(|error| AppError::new(error.to_string()))?;
    let mut results = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|error| AppError::new(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };

        for (index, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                results.push(format!(
                    "{}:{}: {}",
                    entry.path().display(),
                    index + 1,
                    line.trim()
                ));
            }
        }
    }
    Ok(results)
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
}
