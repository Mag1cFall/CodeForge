use std::cmp::Ordering;
use std::path::Path;
use std::sync::Arc;

use crate::error::AppResult;

use super::embedder::{Embedder, create_embedder};
use super::indexer::CodeIndexer;
use super::store::{KnowledgeRepo, KnowledgeStore};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub file_path: String,
    pub content: String,
    pub score: f32,
}

#[derive(Clone)]
pub struct KnowledgeService {
    store: KnowledgeStore,
    indexer: CodeIndexer,
    embedder: Arc<Box<dyn Embedder>>,
}

impl std::fmt::Debug for KnowledgeService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnowledgeService").finish()
    }
}

impl KnowledgeService {
    pub fn new(store: KnowledgeStore) -> Self {
        Self {
            store,
            indexer: CodeIndexer::default(),
            embedder: Arc::new(create_embedder()),
        }
    }

    pub fn list_repos(&self) -> AppResult<Vec<KnowledgeRepo>> {
        self.store.list_repos()
    }

    pub fn index_repo(&self, path: &Path) -> AppResult<KnowledgeRepo> {
        let chunks = self.indexer.index_path(path)?;
        let repo = self.store.upsert_repo(path, "indexed", chunks.len())?;
        let embedded = chunks
            .into_iter()
            .map(|chunk| {
                let vector = self.embedder.embed(&chunk.content)?;
                Ok((chunk, vector))
            })
            .collect::<AppResult<Vec<_>>>()?;
        self.store.replace_chunks(&repo.id, embedded)?;
        self.store.upsert_repo(path, "ready", repo.chunk_count)
    }

    pub fn search(&self, query: &str, top_k: usize) -> AppResult<Vec<SearchResult>> {
        let query_vector = self.embedder.embed(query)?;
        let mut results = self
            .store
            .list_repos()?
            .into_iter()
            .flat_map(|repo| self.store.list_chunks(&repo.id).unwrap_or_default())
            .map(|chunk| SearchResult {
                file_path: chunk.file_path,
                content: chunk.content,
                score: cosine_similarity(&query_vector, &chunk.vector),
            })
            .collect::<Vec<_>>();

        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
        });
        results.truncate(top_k);
        Ok(results)
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || right.is_empty() || left.len() != right.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;
    for (lhs, rhs) in left.iter().zip(right.iter()) {
        dot += lhs * rhs;
        left_norm += lhs * lhs;
        right_norm += rhs * rhs;
    }

    if left_norm == 0.0 || right_norm == 0.0 {
        return 0.0;
    }
    dot / (left_norm.sqrt() * right_norm.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::Database;

    #[test]
    fn indexes_and_searches_repo() {
        let db_path =
            std::env::temp_dir().join(format!("codeforge-kb-{}.db", uuid::Uuid::new_v4()));
        let repo_dir =
            std::env::temp_dir().join(format!("codeforge-kb-src-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&repo_dir).expect("repo dir should exist");
        std::fs::write(
            repo_dir.join("agent.rs"),
            "agent loop tool call context compression",
        )
        .expect("repo file should exist");

        let store = KnowledgeStore::new(Database::new(&db_path).expect("db should initialize"));
        let service = KnowledgeService::new(store);
        service
            .index_repo(&repo_dir)
            .expect("repo should be indexed");

        let results = service.search("agent loop", 5).expect("search should work");
        assert!(!results.is_empty());
        assert!(results[0].file_path.contains("agent.rs"));
    }
}
