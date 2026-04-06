use tauri::State;

use crate::error::IntoCommandResult;
use crate::knowledge::{retriever::SearchResult, store::KnowledgeRepo};
use crate::state::AppState;

#[tauri::command]
pub fn knowledge_repos(state: State<'_, AppState>) -> Result<Vec<KnowledgeRepo>, String> {
    state.knowledge.list_repos().into_command_result()
}

#[tauri::command]
pub fn knowledge_index(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let repo = state
        .knowledge
        .index_repo(std::path::Path::new(&path))
        .into_command_result()?;
    state
        .logs
        .record(
            "knowledge_index",
            serde_json::json!({ "path": path, "chunkCount": repo.chunk_count }),
        )
        .map_err(|error| error.message)?;
    Ok(())
}

#[tauri::command]
pub fn knowledge_search(
    state: State<'_, AppState>,
    query: String,
    top_k: usize,
) -> Result<Vec<SearchResult>, String> {
    let results = state
        .knowledge
        .search(&query, top_k)
        .into_command_result()?;
    state
        .logs
        .record(
            "knowledge_search",
            serde_json::json!({ "query": query, "topK": top_k, "results": results.len() }),
        )
        .map_err(|error| error.message)?;
    Ok(results)
}
