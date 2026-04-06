use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::error::{AppError, AppResult};

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
            chunk_size: 800,
            chunk_overlap: 120,
        }
    }
}

impl CodeIndexer {
    pub fn index_path(&self, root: &Path) -> AppResult<Vec<IndexedChunk>> {
        let mut chunks = Vec::new();
        for entry in WalkDir::new(root) {
            let entry = entry.map_err(|error| AppError::new(error.to_string()))?;
            let path = entry.path();
            if !entry.file_type().is_file() || is_ignored(path) {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };

            let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
            for chunk in chunk_text(&content, self.chunk_size, self.chunk_overlap) {
                let token_count = chunk.split_whitespace().count();
                chunks.push(IndexedChunk {
                    file_path: relative.clone(),
                    content: chunk,
                    token_count,
                });
            }
        }
        Ok(chunks)
    }
}

fn is_ignored(path: &Path) -> bool {
    let text = path.to_string_lossy();
    [".git", "node_modules", "target", "dist", "build"]
        .iter()
        .any(|segment| text.contains(segment))
}

fn chunk_text(text: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<String> {
    if text.len() <= chunk_size {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    while start < bytes.len() {
        while start > 0 && !text.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = (start + chunk_size).min(bytes.len());
        while end < bytes.len() && !text.is_char_boundary(end) {
            end += 1;
        }
        let slice = &text[start..end];
        result.push(slice.to_string());
        if end == bytes.len() {
            break;
        }
        start = end.saturating_sub(chunk_overlap);
    }
    result
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
}
