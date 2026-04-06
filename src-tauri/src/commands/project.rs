use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};
use walkdir::WalkDir;

use crate::state::AppState;
use crate::tools::analysis_tools::{find_code_smells, suggest_refactor};

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
        serde_json::json!({ "step": "scan", "log": format!("扫描 {}", review_root.display()) }),
    )
    .map_err(|error| error.to_string())?;

    let issues = collect_review_issues(&review_root).map_err(|error| error.message.clone())?;

    app.emit(
        "review_progress",
        serde_json::json!({ "step": "complete", "log": format!("发现 {} 个问题", issues.len()) }),
    )
    .map_err(|error| error.to_string())?;
    app.emit("review_result", &issues)
        .map_err(|error| error.to_string())?;
    state
        .logs
        .record("project_review", serde_json::json!({ "path": review_root.display().to_string(), "issueCount": issues.len(), "sandbox": sandbox }))
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
    let smells = find_code_smells(review_root)?;
    let suggestions = suggest_refactor(review_root)?;

    Ok(smells
        .iter()
        .enumerate()
        .map(|(index, smell)| ReviewIssue {
            file: smell["file"].as_str().unwrap_or_default().to_string(),
            line: smell["line"].as_u64().unwrap_or_default() as usize,
            rule: smell["rule"].as_str().unwrap_or_default().to_string(),
            severity: if smell["rule"].as_str() == Some("panic!") {
                "error".into()
            } else {
                "warning".into()
            },
            message: smell["message"].as_str().unwrap_or_default().to_string(),
            suggestion: suggestions
                .get(index)
                .cloned()
                .unwrap_or_else(|| smell["suggestion"].as_str().unwrap_or_default().to_string()),
        })
        .collect())
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
