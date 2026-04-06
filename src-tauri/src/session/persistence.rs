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

    pub fn load_sessions(&self) -> AppResult<Vec<SessionRecord>> {
        self.manager.list()
    }

    pub fn load_messages(&self, session_id: &str) -> AppResult<Vec<SessionMessage>> {
        self.manager.messages(session_id)
    }
}
