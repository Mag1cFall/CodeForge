use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use crate::error::{AppError, AppResult};

use super::embedder::{create_embedder, Embedder, EmbedderInfo};
use super::indexer::CodeIndexer;
use super::knowledge_log;
use super::store::{KnowledgeChunkRecord, KnowledgeRepo, KnowledgeStore};

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
    embedder: Option<Arc<Box<dyn Embedder>>>,
    embedder_info: Option<EmbedderInfo>,
}

impl std::fmt::Debug for KnowledgeService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnowledgeService")
            .field("embedder", &self.embedder_info)
            .finish()
    }
}

impl KnowledgeService {
    pub fn new(store: KnowledgeStore) -> Self {
        let (embedder, embedder_info) = match create_embedder() {
            Ok(Some(embedder)) => {
                let info = embedder.info();
                (Some(Arc::new(embedder)), Some(info))
            }
            Ok(None) => (None, None),
            Err(error) => {
                knowledge_log(
                    "retriever.embedder.init_failed",
                    serde_json::json!({ "error": error.message }),
                );
                (None, None)
            }
        };

        Self {
            store,
            indexer: CodeIndexer::default(),
            embedder,
            embedder_info,
        }
    }

    pub fn list_repos(&self) -> AppResult<Vec<KnowledgeRepo>> {
        self.store.list_repos()
    }

    pub fn index_repo(&self, path: &Path) -> AppResult<KnowledgeRepo> {
        if !path.exists() {
            return Err(AppError::new(format!("目录不存在: {}", path.display())));
        }
        if !path.is_dir() {
            return Err(AppError::new(format!("不是目录: {}", path.display())));
        }

        knowledge_log(
            "retriever.index.start",
            serde_json::json!({
                "path": path.display().to_string(),
                "mode": if self.embedder.is_some() { "hybrid" } else { "sparse-only" },
                "embedder": self.embedder_info,
            }),
        );

        let _ = self.store.upsert_repo(path, "indexing", 0)?;
        let chunks = match self.indexer.index_path(path) {
            Ok(chunks) => chunks,
            Err(error) => {
                let _ = self.store.upsert_repo(path, "error", 0);
                knowledge_log(
                    "retriever.index.failed",
                    serde_json::json!({
                        "path": path.display().to_string(),
                        "stage": "index_path",
                        "error": error.message,
                    }),
                );
                return Err(error);
            }
        };

        let chunk_count = chunks.len();
        let mut embedded_chunks = Vec::with_capacity(chunk_count);
        if let Some(embedder) = &self.embedder {
            for (index, chunk) in chunks.into_iter().enumerate() {
                let vector = embedder.embed(&chunk.content).map_err(|error| {
                    AppError::new(format!(
                        "为分块生成向量失败: file={}, index={}, error={}",
                        chunk.file_path.display(),
                        index,
                        error.message
                    ))
                })?;
                embedded_chunks.push((chunk, vector));

                if (index + 1) % 100 == 0 || index + 1 == chunk_count {
                    knowledge_log(
                        "retriever.index.embedding_progress",
                        serde_json::json!({
                            "path": path.display().to_string(),
                            "embedded": index + 1,
                            "total": chunk_count,
                        }),
                    );
                }
            }
        } else {
            for chunk in chunks {
                embedded_chunks.push((chunk, Vec::new()));
            }
        }

        if let Err(error) = self.store.replace_chunks(
            &self.store.upsert_repo(path, "indexing", chunk_count)?.id,
            embedded_chunks,
        ) {
            let _ = self.store.upsert_repo(path, "error", 0);
            knowledge_log(
                "retriever.index.failed",
                serde_json::json!({
                    "path": path.display().to_string(),
                    "stage": "replace_chunks",
                    "error": error.message,
                }),
            );
            return Err(error);
        }

        let repo = self.store.upsert_repo(path, "ready", chunk_count)?;
        knowledge_log(
            "retriever.index.complete",
            serde_json::json!({
                "repoId": repo.id,
                "path": repo.path,
                "chunkCount": repo.chunk_count,
                "mode": if self.embedder.is_some() { "hybrid" } else { "sparse-only" },
            }),
        );
        Ok(repo)
    }

    pub fn search(&self, query: &str, top_k: usize) -> AppResult<Vec<SearchResult>> {
        let cleaned_query = query.trim();
        if cleaned_query.is_empty() || top_k == 0 {
            return Ok(Vec::new());
        }

        let safe_top_k = top_k.min(100);
        let candidate_limit = safe_top_k.saturating_mul(8).max(safe_top_k);
        let chunks = self.store.list_all_chunks()?;
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let query_terms = extract_query_terms(cleaned_query);
        let sparse_ranked = sparse_rank_chunks(&chunks, &query_terms);
        let sparse_by_id = sparse_ranked
            .iter()
            .map(|(id, score)| (id.clone(), *score))
            .collect::<HashMap<_, _>>();

        let mut vector_by_id = HashMap::<String, f32>::new();
        let mut vector_available = false;
        let mut vector_mode = "not-enabled";
        if let Some(embedder) = &self.embedder {
            match embedder.embed(cleaned_query) {
                Ok(query_vector) if !query_vector.is_empty() => {
                    vector_available = true;
                    vector_mode = "enabled";
                    let mut ranked = chunks
                        .iter()
                        .filter_map(|chunk| {
                            let score = normalized_cosine_similarity(&query_vector, &chunk.vector);
                            if score <= 0.0 {
                                return None;
                            }
                            Some((chunk.id.clone(), score))
                        })
                        .collect::<Vec<_>>();
                    ranked.sort_by(|left, right| {
                        right.1.partial_cmp(&left.1).unwrap_or(Ordering::Equal)
                    });
                    ranked.truncate(candidate_limit);
                    for (id, score) in ranked {
                        vector_by_id.insert(id, score);
                    }
                }
                Ok(_) => {
                    vector_mode = "query-vector-empty";
                }
                Err(error) => {
                    vector_mode = "query-embedding-failed";
                    knowledge_log(
                        "retriever.search.embedding_failed",
                        serde_json::json!({
                            "error": error.message,
                            "queryChars": cleaned_query.chars().count(),
                        }),
                    );
                }
            }
        }

        let mut candidate_ids = HashSet::<String>::new();
        if vector_available {
            for id in vector_by_id.keys() {
                candidate_ids.insert(id.clone());
            }
            for (id, _) in sparse_ranked.iter().take(candidate_limit) {
                candidate_ids.insert(id.clone());
            }
        } else {
            for (id, _) in sparse_ranked.iter().take(candidate_limit) {
                candidate_ids.insert(id.clone());
            }
        }

        if candidate_ids.is_empty() {
            knowledge_log(
                "retriever.search.complete",
                serde_json::json!({
                    "mode": if vector_available { "hybrid" } else { "sparse-only" },
                    "queryChars": cleaned_query.chars().count(),
                    "queryTerms": query_terms.len(),
                    "chunks": chunks.len(),
                    "results": 0,
                    "vectorMode": vector_mode,
                }),
            );
            return Ok(Vec::new());
        }

        let chunk_map = chunks
            .iter()
            .map(|chunk| (chunk.id.as_str(), chunk))
            .collect::<HashMap<_, _>>();

        let mut scored_results = Vec::new();
        for chunk_id in candidate_ids {
            let Some(chunk) = chunk_map.get(chunk_id.as_str()) else {
                continue;
            };
            let vector_score = vector_by_id.get(&chunk_id).copied().unwrap_or(0.0);
            let text_score = sparse_by_id.get(&chunk_id).copied().unwrap_or(0.0);

            let final_score = if vector_available {
                if query_terms.is_empty() || text_score == 0.0 {
                    vector_score
                } else {
                    0.7 * vector_score + 0.3 * text_score
                }
            } else {
                text_score
            };

            if final_score <= 0.0 {
                continue;
            }

            scored_results.push(SearchResult {
                file_path: chunk.file_path.clone(),
                content: chunk.content.clone(),
                score: final_score,
            });
        }

        scored_results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
        });
        scored_results.truncate(safe_top_k);

        knowledge_log(
            "retriever.search.complete",
            serde_json::json!({
                "mode": if vector_available {
                    if query_terms.is_empty() { "dense-only" } else { "hybrid" }
                } else {
                    "sparse-only"
                },
                "queryChars": cleaned_query.chars().count(),
                "queryTerms": query_terms.len(),
                "chunks": chunks.len(),
                "candidateCount": candidate_limit,
                "results": scored_results.len(),
                "vectorMode": vector_mode,
            }),
        );

        Ok(scored_results)
    }
}

fn extract_query_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    for token in tokenize(query) {
        if is_stop_word(&token) || !is_valid_keyword(&token) {
            continue;
        }
        if seen.insert(token.clone()) {
            terms.push(token);
        }
    }
    terms
}

fn sparse_rank_chunks(
    chunks: &[KnowledgeChunkRecord],
    query_terms: &[String],
) -> Vec<(String, f32)> {
    if chunks.is_empty() || query_terms.is_empty() {
        return Vec::new();
    }

    let tokenized_docs = chunks
        .iter()
        .map(|chunk| tokenize(&chunk.content))
        .collect::<Vec<_>>();
    let total_docs = tokenized_docs.len() as f32;
    let avg_doc_len = tokenized_docs
        .iter()
        .map(|tokens| tokens.len().max(1) as f32)
        .sum::<f32>()
        / total_docs.max(1.0);

    let mut document_frequency = HashMap::<String, usize>::new();
    for term in query_terms {
        let count = tokenized_docs
            .iter()
            .filter(|tokens| tokens.iter().any(|token| token == term))
            .count();
        document_frequency.insert(term.clone(), count);
    }

    let k1 = 1.2f32;
    let b = 0.75f32;
    let mut ranked = Vec::new();

    for (chunk, tokens) in chunks.iter().zip(tokenized_docs.iter()) {
        let mut term_frequency = HashMap::<String, usize>::new();
        for token in tokens {
            *term_frequency.entry(token.clone()).or_insert(0) += 1;
        }

        let doc_len = tokens.len().max(1) as f32;
        let mut bm25_score = 0.0f32;
        for term in query_terms {
            let tf = term_frequency.get(term).copied().unwrap_or(0) as f32;
            if tf == 0.0 {
                continue;
            }

            let df = document_frequency.get(term).copied().unwrap_or(0) as f32;
            let idf = (((total_docs - df + 0.5) / (df + 0.5)) + 1.0).ln();
            let denominator = tf + k1 * (1.0 - b + b * doc_len / avg_doc_len.max(1.0));
            bm25_score += idf * ((tf * (k1 + 1.0)) / denominator.max(1e-6));
        }

        if bm25_score > 0.0 {
            let normalized = bm25_score / (1.0 + bm25_score);
            ranked.push((chunk.id.clone(), normalized));
        }
    }

    ranked.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(Ordering::Equal));
    ranked
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut ascii = String::new();
    let mut cjk = Vec::<char>::new();

    for ch in text.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            flush_cjk_tokens(&mut cjk, &mut tokens);
            ascii.push(ch);
            continue;
        }

        if is_cjk(ch) {
            flush_ascii_token(&mut ascii, &mut tokens);
            cjk.push(ch);
            continue;
        }

        flush_ascii_token(&mut ascii, &mut tokens);
        flush_cjk_tokens(&mut cjk, &mut tokens);
    }

    flush_ascii_token(&mut ascii, &mut tokens);
    flush_cjk_tokens(&mut cjk, &mut tokens);
    tokens
}

fn flush_ascii_token(buffer: &mut String, tokens: &mut Vec<String>) {
    if buffer.is_empty() {
        return;
    }
    tokens.push(std::mem::take(buffer));
}

fn flush_cjk_tokens(buffer: &mut Vec<char>, tokens: &mut Vec<String>) {
    if buffer.is_empty() {
        return;
    }
    for ch in buffer.iter() {
        tokens.push(ch.to_string());
    }
    for pair in buffer.windows(2) {
        if let [left, right] = pair {
            tokens.push(format!("{}{}", left, right));
        }
    }
    buffer.clear();
}

fn is_cjk(ch: char) -> bool {
    let value = ch as u32;
    (0x4E00..=0x9FFF).contains(&value)
        || (0x3400..=0x4DBF).contains(&value)
        || (0x3040..=0x30FF).contains(&value)
        || (0xAC00..=0xD7AF).contains(&value)
        || (0x3131..=0x3163).contains(&value)
}

fn is_valid_keyword(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    if token.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    if token.is_ascii() && token.len() < 2 {
        return false;
    }
    true
}

fn is_stop_word(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "the"
            | "this"
            | "that"
            | "these"
            | "those"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "being"
            | "of"
            | "for"
            | "to"
            | "in"
            | "on"
            | "at"
            | "with"
            | "and"
            | "or"
            | "but"
            | "if"
            | "then"
            | "what"
            | "which"
            | "who"
            | "how"
            | "why"
            | "请"
            | "帮"
            | "一下"
            | "这个"
            | "那个"
            | "什么"
            | "怎么"
            | "为什么"
    )
}

fn normalized_cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let cosine = cosine_similarity(left, right);
    ((cosine + 1.0) / 2.0).clamp(0.0, 1.0)
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

    if left_norm <= 0.0 || right_norm <= 0.0 {
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

    #[test]
    fn extracts_keywords_from_query() {
        let terms = extract_query_terms("请帮我找到 agent loop 的实现");
        assert!(terms.contains(&"agent".to_string()));
    }
}
