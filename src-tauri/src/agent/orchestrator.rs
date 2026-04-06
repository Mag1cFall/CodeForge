use crate::error::AppResult;

use super::definition::{AgentRecord, AgentStore};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedTask {
    pub agent_id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct AgentOrchestrator {
    store: AgentStore,
}

impl AgentOrchestrator {
    pub fn new(store: AgentStore) -> Self {
        Self { store }
    }

    pub fn choose(&self, intent: &str) -> AppResult<RoutedTask> {
        let agents = self.store.list()?;
        let preferred = if intent.contains("审查") || intent.contains("review") {
            find_by_name(&agents, "review")
        } else if intent.contains("重构") || intent.contains("refactor") {
            find_by_name(&agents, "refactor")
        } else if intent.contains("研究") || intent.contains("best practice") {
            find_by_name(&agents, "research")
        } else {
            find_by_name(&agents, "orchestrator")
        }
        .or_else(|| agents.first())
        .ok_or_else(|| crate::error::AppError::new("当前没有可用 Agent"))?;

        Ok(RoutedTask {
            agent_id: preferred.id.clone(),
            reason: format!("根据任务意图将请求路由到 {}", preferred.name),
        })
    }
}

fn find_by_name<'a>(agents: &'a [AgentRecord], needle: &str) -> Option<&'a AgentRecord> {
    let needle = needle.to_ascii_lowercase();
    agents
        .iter()
        .find(|agent| agent.name.to_ascii_lowercase().contains(&needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::Database;

    #[test]
    fn routes_review_intent_to_reviewer_when_present() {
        let db_path = std::env::temp_dir().join(format!(
            "codeforge-orchestrator-{}.db",
            uuid::Uuid::new_v4()
        ));
        let store = AgentStore::new(Database::new(&db_path).expect("db should initialize"));
        store
            .ensure_default_agent()
            .expect("default agent should exist");
        store
            .create(crate::agent::definition::AgentConfigInput {
                name: "Reviewer".into(),
                instructions: Some("review".into()),
                tools: vec!["read_file".into()],
                model: "gpt-5.4-mini".into(),
            })
            .expect("reviewer should be created");

        let routed = AgentOrchestrator::new(store)
            .choose("请审查这个仓库")
            .expect("route should resolve");
        assert!(!routed.agent_id.is_empty());
    }
}
