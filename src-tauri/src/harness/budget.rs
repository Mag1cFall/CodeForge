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
        if tokens == 0 {
            log_budget_event(
                "reserve_skip_zero",
                serde_json::json!({
                    "sessionLimit": self.session_limit,
                    "globalLimit": self.global_limit,
                }),
            );
            return Ok(());
        }

        let Some(next_session) = reserve_counter(&self.session_used, self.session_limit, tokens)
        else {
            log_budget_event(
                "reserve_rejected_session",
                serde_json::json!({
                    "tokens": tokens,
                    "sessionUsed": self.session_used.load(Ordering::SeqCst),
                    "sessionLimit": self.session_limit,
                }),
            );
            return Err(AppError::new("Token 会话预算已超限"));
        };

        let Some(next_global) = reserve_counter(&self.global_used, self.global_limit, tokens)
        else {
            self.session_used.fetch_sub(tokens, Ordering::SeqCst);
            log_budget_event(
                "reserve_rejected_global",
                serde_json::json!({
                    "tokens": tokens,
                    "globalUsed": self.global_used.load(Ordering::SeqCst),
                    "globalLimit": self.global_limit,
                    "sessionRolledBackTo": self.session_used.load(Ordering::SeqCst),
                }),
            );
            return Err(AppError::new("Token 全局预算已超限"));
        };

        log_budget_event(
            "reserve_success",
            serde_json::json!({
                "tokens": tokens,
                "sessionUsed": next_session,
                "sessionLimit": self.session_limit,
                "globalUsed": next_global,
                "globalLimit": self.global_limit,
            }),
        );
        Ok(())
    }

    pub fn release(&self, tokens: usize) {
        if tokens == 0 {
            return;
        }

        let _ = self
            .session_used
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(tokens))
            });
        let _ = self
            .global_used
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(tokens))
            });

        log_budget_event(
            "release",
            serde_json::json!({
                "tokens": tokens,
                "sessionUsed": self.session_used.load(Ordering::SeqCst),
                "globalUsed": self.global_used.load(Ordering::SeqCst),
            }),
        );
    }

    pub fn snapshot(&self) -> (usize, usize) {
        (
            self.session_used.load(Ordering::SeqCst),
            self.global_used.load(Ordering::SeqCst),
        )
    }
}

fn reserve_counter(counter: &AtomicUsize, limit: usize, tokens: usize) -> Option<usize> {
    loop {
        let current = counter.load(Ordering::SeqCst);
        let next = current.checked_add(tokens)?;
        if next > limit {
            return None;
        }

        if counter
            .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return Some(next);
        }
    }
}

fn log_budget_event(event: &str, payload: serde_json::Value) {
    eprintln!(
        "{}",
        serde_json::json!({
            "component": "harness.budget",
            "event": event,
            "payload": payload,
        })
    );
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

    #[test]
    fn rollback_usage_when_reserve_fails() {
        let budget = TokenBudget::new(10, 10);
        assert!(budget.reserve(9).is_ok());
        assert!(budget.reserve(2).is_err());
        assert_eq!(budget.snapshot(), (9, 9));
    }
}
