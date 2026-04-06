use std::path::Path;

use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::sqlite::Database;
use crate::error::{AppError, AppResult};

use super::indexer::IndexedChunk;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeRepo {
    pub id: String,
    pub path: String,
    pub status: String,
    pub chunk_count: usize,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeChunkRecord {
    pub id: String,
    pub repo_id: String,
    pub file_path: String,
    pub content: String,
    pub token_count: usize,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct KnowledgeStore {
    db: Database,
}

impl KnowledgeStore {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn upsert_repo(
        &self,
        path: &Path,
        status: &str,
        chunk_count: usize,
    ) -> AppResult<KnowledgeRepo> {
        let existing = self.get_repo_by_path(path)?;
        let repo_id = existing
            .as_ref()
            .map(|repo| repo.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = chrono::Utc::now().to_rfc3339();

        let connection = self.db.connection()?;
        connection.execute(
            r#"
            INSERT INTO knowledge_repos (id, path, status, chunk_count, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(path) DO UPDATE SET
                status = excluded.status,
                chunk_count = excluded.chunk_count,
                updated_at = excluded.updated_at
            "#,
            params![
                repo_id,
                path.display().to_string(),
                status,
                chunk_count as i64,
                now
            ],
        )?;

        self.get_repo_by_id(&repo_id)?
            .ok_or_else(|| AppError::new("未能读取知识库索引记录"))
    }

    pub fn replace_chunks(
        &self,
        repo_id: &str,
        chunks: Vec<(IndexedChunk, Vec<f32>)>,
    ) -> AppResult<()> {
        let mut connection = self.db.connection()?;
        let tx = connection.transaction()?;
        tx.execute(
            "DELETE FROM knowledge_chunks WHERE repo_id = ?1",
            params![repo_id],
        )?;
        for (chunk, vector) in chunks {
            tx.execute(
                r#"
                INSERT INTO knowledge_chunks (id, repo_id, file_path, content, token_count, vector_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    Uuid::new_v4().to_string(),
                    repo_id,
                    chunk.file_path.display().to_string(),
                    chunk.content,
                    chunk.token_count as i64,
                    serde_json::to_string(&vector)?,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_repos(&self) -> AppResult<Vec<KnowledgeRepo>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, path, status, chunk_count, updated_at FROM knowledge_repos ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(KnowledgeRepo {
                id: row.get(0)?,
                path: row.get(1)?,
                status: row.get(2)?,
                chunk_count: row.get::<_, i64>(3)? as usize,
                updated_at: row.get(4)?,
            })
        })?;
        let mut repos = Vec::new();
        for row in rows {
            repos.push(row?);
        }
        Ok(repos)
    }

    pub fn list_chunks(&self, repo_id: &str) -> AppResult<Vec<KnowledgeChunkRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, repo_id, file_path, content, token_count, vector_json FROM knowledge_chunks WHERE repo_id = ?1",
        )?;
        let rows = statement.query_map(params![repo_id], |row| {
            Ok(KnowledgeChunkRecord {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                file_path: row.get(2)?,
                content: row.get(3)?,
                token_count: row.get::<_, i64>(4)? as usize,
                vector: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
            })
        })?;

        let mut chunks = Vec::new();
        for row in rows {
            chunks.push(row?);
        }
        Ok(chunks)
    }

    fn get_repo_by_path(&self, path: &Path) -> AppResult<Option<KnowledgeRepo>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, path, status, chunk_count, updated_at FROM knowledge_repos WHERE path = ?1 LIMIT 1",
        )?;
        let repo = statement
            .query_row(params![path.display().to_string()], |row| {
                Ok(KnowledgeRepo {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    status: row.get(2)?,
                    chunk_count: row.get::<_, i64>(3)? as usize,
                    updated_at: row.get(4)?,
                })
            })
            .optional()?;
        Ok(repo)
    }

    fn get_repo_by_id(&self, id: &str) -> AppResult<Option<KnowledgeRepo>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, path, status, chunk_count, updated_at FROM knowledge_repos WHERE id = ?1 LIMIT 1",
        )?;
        let repo = statement
            .query_row(params![id], |row| {
                Ok(KnowledgeRepo {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    status: row.get(2)?,
                    chunk_count: row.get::<_, i64>(3)? as usize,
                    updated_at: row.get(4)?,
                })
            })
            .optional()?;
        Ok(repo)
    }
}
