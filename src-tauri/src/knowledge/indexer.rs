use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::error::{AppError, AppResult};

use super::knowledge_log;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedChunk {
    pub file_path: PathBuf,
    pub content: String,
    pub token_count: usize,
}

#[derive(Debug, Clone)]
pub struct CodeIndexer {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
}

impl Default for CodeIndexer {
    fn default() -> Self {
        Self {
            chunk_size: 512,
            chunk_overlap: 80,
        }
    }
}

impl CodeIndexer {
    pub fn index_path(&self, root: &Path) -> AppResult<Vec<IndexedChunk>> {
        if !root.exists() {
            return Err(AppError::new(format!("索引路径不存在: {}", root.display())));
        }
        if !root.is_dir() {
            return Err(AppError::new(format!(
                "索引路径不是目录: {}",
                root.display()
            )));
        }

        let mut chunks = Vec::new();
        let mut indexed_files = 0usize;
        let mut skipped_files = 0usize;

        for entry in WalkDir::new(root).follow_links(false) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    skipped_files += 1;
                    knowledge_log(
                        "indexer.walk.error",
                        serde_json::json!({
                            "root": root.display().to_string(),
                            "error": error.to_string(),
                        }),
                    );
                    continue;
                }
            };
            let path = entry.path();
            if !entry.file_type().is_file() || is_ignored(path) {
                continue;
            }

            let bytes = match std::fs::read(path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    skipped_files += 1;
                    knowledge_log(
                        "indexer.file.read_skipped",
                        serde_json::json!({
                            "path": path.display().to_string(),
                            "reason": error.to_string(),
                        }),
                    );
                    continue;
                }
            };

            if looks_like_binary(&bytes) {
                skipped_files += 1;
                continue;
            }

            let content = match String::from_utf8(bytes) {
                Ok(content) => content,
                Err(_) => {
                    skipped_files += 1;
                    continue;
                }
            };

            let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
            for chunk in chunk_text(&content, self.chunk_size, self.chunk_overlap) {
                if chunk.trim().is_empty() {
                    continue;
                }
                let token_count = estimate_token_count(&chunk);
                chunks.push(IndexedChunk {
                    file_path: relative.clone(),
                    content: chunk,
                    token_count,
                });
            }
            indexed_files += 1;
        }

        knowledge_log(
            "indexer.index_path.complete",
            serde_json::json!({
                "root": root.display().to_string(),
                "indexedFiles": indexed_files,
                "skippedFiles": skipped_files,
                "chunkCount": chunks.len(),
                "chunkSize": self.chunk_size,
                "chunkOverlap": self.chunk_overlap,
            }),
        );

        Ok(chunks)
    }
}

fn is_ignored(path: &Path) -> bool {
    path.components().any(|segment| {
        matches!(
            segment.as_os_str().to_str(),
            Some(".git" | "node_modules" | "target" | "dist" | "build")
        )
    })
}

fn chunk_text(text: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    if chunk_size == 0 {
        return vec![text.to_string()];
    }

    let overlap = chunk_overlap.min(chunk_size.saturating_sub(1));
    let separators = ["\n\n", "\n", "。", "，", ". ", ", ", " ", ""];
    recursive_chunk_text(text, chunk_size, overlap, &separators)
        .into_iter()
        .filter(|chunk| !chunk.trim().is_empty())
        .collect()
}

fn recursive_chunk_text(
    text: &str,
    chunk_size: usize,
    chunk_overlap: usize,
    separators: &[&str],
) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    if text.chars().count() <= chunk_size {
        return vec![text.to_string()];
    }

    for separator in separators {
        if separator.is_empty() {
            return split_by_character(text, chunk_size, chunk_overlap);
        }
        if !text.contains(separator) {
            continue;
        }

        let splits = split_keep_separator(text, separator);
        if splits.len() <= 1 {
            continue;
        }

        let mut result = Vec::new();
        let mut current = String::new();
        let mut current_len = 0usize;

        for split in splits {
            let split_len = split.chars().count();
            if split_len > chunk_size {
                if !current.is_empty() {
                    result.extend(recursive_chunk_text(
                        &current,
                        chunk_size,
                        chunk_overlap,
                        separators,
                    ));
                    current.clear();
                    current_len = 0;
                }
                result.extend(recursive_chunk_text(
                    &split,
                    chunk_size,
                    chunk_overlap,
                    separators,
                ));
                continue;
            }

            if current_len + split_len > chunk_size && !current.is_empty() {
                result.push(current.clone());
                let overlap_text = tail_chars(&current, chunk_overlap);
                current.clear();
                current.push_str(&overlap_text);
                current.push_str(&split);
                current_len = current.chars().count();
            } else {
                current.push_str(&split);
                current_len += split_len;
            }
        }

        if !current.is_empty() {
            result.push(current);
        }
        return result;
    }

    vec![text.to_string()]
}

fn split_keep_separator(text: &str, separator: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut remaining = text;

    while let Some(index) = remaining.find(separator) {
        let end = index + separator.len();
        result.push(remaining[..end].to_string());
        remaining = &remaining[end..];
    }

    if !remaining.is_empty() {
        result.push(remaining.to_string());
    }

    result
}

fn split_by_character(text: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<String> {
    if chunk_size == 0 {
        return vec![text.to_string()];
    }

    let chars = text.chars().collect::<Vec<_>>();
    if chars.len() <= chunk_size {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let step = chunk_size.saturating_sub(chunk_overlap).max(1);
    let mut start = 0usize;

    while start < chars.len() {
        let end = (start + chunk_size).min(chars.len());
        result.push(chars[start..end].iter().collect::<String>());
        if end == chars.len() {
            break;
        }
        start = start.saturating_add(step);
    }

    result
}

fn tail_chars(text: &str, count: usize) -> String {
    if count == 0 {
        return String::new();
    }
    let total = text.chars().count();
    if total <= count {
        return text.to_string();
    }

    let start_index = total - count;
    let start_byte = text
        .char_indices()
        .nth(start_index)
        .map(|(index, _)| index)
        .unwrap_or(0);
    text[start_byte..].to_string()
}

fn looks_like_binary(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(1_024)];
    sample.iter().any(|byte| *byte == 0)
}

fn estimate_token_count(text: &str) -> usize {
    let whitespace_tokens = text.split_whitespace().count();
    if whitespace_tokens > 0 {
        return whitespace_tokens;
    }
    text.chars().count().div_ceil(4).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_source_files_into_chunks() {
        let dir = std::env::temp_dir().join(format!("codeforge-indexer-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should exist");
        std::fs::write(
            dir.join("main.rs"),
            "fn main() { println!(\"hello\"); }".repeat(80),
        )
        .expect("source file should exist");

        let chunks = CodeIndexer::default()
            .index_path(&dir)
            .expect("index should succeed");
        assert!(!chunks.is_empty());
    }

    #[test]
    fn chunks_text_with_overlap() {
        let text = "alpha beta gamma delta epsilon".repeat(30);
        let chunks = chunk_text(&text, 120, 20);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| !chunk.trim().is_empty()));
    }
}
