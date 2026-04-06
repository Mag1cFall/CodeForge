use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::harness::hashline::annotate_text;

pub fn read_file(path: impl AsRef<Path>) -> AppResult<String> {
    let content = std::fs::read_to_string(path.as_ref())?;
    Ok(annotate_text(&content))
}

pub fn list_directory(path: impl AsRef<Path>) -> AppResult<Vec<String>> {
    let mut entries = std::fs::read_dir(path.as_ref())?
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                format!("{}/", entry.file_name().to_string_lossy())
            } else {
                entry.file_name().to_string_lossy().to_string()
            }
        })
        .collect::<Vec<_>>();
    entries.sort();
    Ok(entries)
}

pub fn write_file(path: impl AsRef<Path>, content: &str) -> AppResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

pub fn apply_patch_text(path: impl AsRef<Path>, old: &str, new: &str) -> AppResult<String> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)?;
    if !content.contains(old) {
        return Err(AppError::new("补丁目标未匹配到原始内容"));
    }
    let updated = content.replacen(old, new, 1);
    std::fs::write(path, &updated)?;
    Ok(updated)
}

pub fn resolve_path(root: Option<&Path>, input: &str) -> AppResult<PathBuf> {
    let path = PathBuf::from(input);
    if path.is_absolute() {
        return Ok(path);
    }
    if let Some(root) = root {
        return Ok(root.join(path));
    }
    Err(AppError::new("相对路径缺少工作区根目录"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hash_annotated_file() {
        let dir = std::env::temp_dir().join(format!("codeforge-file-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should exist");
        let file = dir.join("sample.txt");
        std::fs::write(&file, "hello\nworld").expect("sample file should exist");

        let content = read_file(&file).expect("file should be readable");
        assert!(content.contains("1#"));
        assert!(content.contains("| hello"));
    }

    #[test]
    fn writes_and_patches_file() {
        let dir =
            std::env::temp_dir().join(format!("codeforge-file-write-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should exist");
        let file = dir.join("sample.txt");
        write_file(&file, "alpha beta").expect("file should be written");
        let updated = apply_patch_text(&file, "beta", "gamma").expect("patch should apply");
        assert!(updated.contains("gamma"));
    }
}
