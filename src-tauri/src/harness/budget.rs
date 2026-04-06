use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct TokenBudget {
    session_limit: usize,
    global_limit: usize,
    session_used: Arc<AtomicUsize>,
    global_used: Arc<AtomicUsize>,
}

impl TokenBudget {
    pub fn new(session_limit: usize, global_limit: usize) -> Self {
        Self {
            session_limit,
            global_limit,
            session_used: Arc::new(AtomicUsize::new(0)),
            global_used: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn reserve(&self, tokens: usize) -> AppResult<()> {
        let next_session = self.session_used.fetch_add(tokens, Ordering::SeqCst) + tokens;
        let next_global = self.global_used.fetch_add(tokens, Ordering::SeqCst) + tokens;
        if next_session > self.session_limit || next_global > self.global_limit {
            return Err(AppError::new("Token 预算已超限"));
        }
        Ok(())
    }

    pub fn snapshot(&self) -> (usize, usize) {
        (
            self.session_used.load(Ordering::SeqCst),
            self.global_used.load(Ordering::SeqCst),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_requests_over_budget() {
        let budget = TokenBudget::new(10, 10);
        assert!(budget.reserve(4).is_ok());
        assert!(budget.reserve(7).is_err());
    }
}
