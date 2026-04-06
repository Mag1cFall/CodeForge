use std::path::Path;

use walkdir::WalkDir;

use crate::error::{AppError, AppResult};

pub fn analyze_ast(path: &Path) -> AppResult<serde_json::Value> {
    let content = std::fs::read_to_string(path)?;
    let line_count = content.lines().count();
    let function_count = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("function ")
                || trimmed.contains(" => ")
        })
        .count();

    Ok(serde_json::json!({
        "file": path.display().to_string(),
        "lineCount": line_count,
        "functionCount": function_count,
        "complexity": check_complexity(path)?,
    }))
}

pub fn check_complexity(path: &Path) -> AppResult<usize> {
    let content = std::fs::read_to_string(path)?;
    let mut score = 1usize;
    for needle in [" if ", " else ", " match ", " for ", " while ", "&&", "||"] {
        score += content.matches(needle).count();
    }
    Ok(score)
}

pub fn find_code_smells(root: &Path) -> AppResult<Vec<serde_json::Value>> {
    let patterns = [
        ("unwrap", "建议用显式错误处理替代 unwrap"),
        ("todo!", "发现占位实现，需要补齐真实逻辑"),
        ("panic!", "发现 panic，建议转换为 Result 或可恢复错误"),
    ];

    let mut results = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|error| AppError::new(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };

        for (line_number, line) in content.lines().enumerate() {
            for (pattern, suggestion) in patterns {
                if line.contains(pattern) {
                    results.push(serde_json::json!({
                        "file": entry.path().display().to_string(),
                        "line": line_number + 1,
                        "rule": pattern,
                        "message": format!("检测到 {}", pattern),
                        "suggestion": suggestion,
                    }));
                }
            }
        }
    }

    Ok(results)
}

pub fn suggest_refactor(root: &Path) -> AppResult<Vec<String>> {
    let smells = find_code_smells(root)?;
    Ok(smells
        .iter()
        .map(|item| {
            format!(
                "{}:{} -> {}",
                item["file"].as_str().unwrap_or_default(),
                item["line"].as_u64().unwrap_or_default(),
                item["suggestion"].as_str().unwrap_or_default()
            )
        })
        .collect())
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
        assert_eq!(smells.len(), 1);
    }
}
