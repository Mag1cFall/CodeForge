use crate::error::AppResult;

use super::manager::{SessionManager, SessionMessage, SessionRecord};

#[derive(Debug, Clone)]
pub struct SessionPersistence {
    manager: SessionManager,
}

impl SessionPersistence {
    pub fn new(manager: SessionManager) -> Self {
        Self { manager }
    }

    pub fn list_sessions(&self) -> AppResult<Vec<SessionRecord>> {
        self.manager.list()
    }

    pub fn load_sessions(&self) -> AppResult<Vec<SessionRecord>> {
        self.list_sessions()
    }

    pub fn create_session(
        &self,
        agent_id: String,
        title: Option<String>,
    ) -> AppResult<SessionRecord> {
        self.manager.create(agent_id, title)
    }

    pub fn create_session_with_context_max(
        &self,
        agent_id: String,
        title: Option<String>,
        context_tokens_max: usize,
    ) -> AppResult<SessionRecord> {
        self.manager
            .create_with_context_max(agent_id, title, context_tokens_max)
    }

    pub fn get_session(&self, session_id: &str) -> AppResult<Option<SessionRecord>> {
        self.manager.get(session_id)
    }

    pub fn delete_session(&self, session_id: &str) -> AppResult<()> {
        self.manager.delete(session_id)
    }

    pub fn load_messages(&self, session_id: &str) -> AppResult<Vec<SessionMessage>> {
        self.manager.messages(session_id)
    }

    pub fn append_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        tool_calls: Vec<serde_json::Value>,
    ) -> AppResult<SessionMessage> {
        self.manager
            .append_message(session_id, role, content, tool_calls)
    }

    pub fn update_usage(&self, session_id: &str, used_tokens: usize) -> AppResult<()> {
        self.manager.update_usage(session_id, used_tokens)
    }

    pub fn update_context_max(&self, session_id: &str, context_tokens_max: usize) -> AppResult<()> {
        self.manager
            .update_context_max(session_id, context_tokens_max)
    }

    pub fn normalize_model_context_max(&self, session_id: &str, model: &str) -> AppResult<()> {
        self.manager.normalize_model_context_max(session_id, model)
    }
}
