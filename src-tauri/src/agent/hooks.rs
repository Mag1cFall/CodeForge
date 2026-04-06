use std::sync::{Arc, Mutex};

use crate::llm::model::ChatResponse;
use crate::logging::service::TraceLogService;
use crate::tools::schema::ToolSchema;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentHooksConfig {
    pub emit_events: bool,
    pub capture_history: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentHookEvent {
    AgentStart {
        session_id: String,
    },
    BeforeLlmCall {
        message_count: usize,
    },
    AfterLlmCall {
        model: String,
        output_tokens: usize,
    },
    BeforeToolExec {
        tool_name: String,
    },
    AfterToolExec {
        tool_name: String,
        output_preview: String,
    },
    AgentEnd {
        session_id: String,
        response_preview: String,
    },
}

pub trait AgentHooks: Send + Sync {
    fn on_agent_start(&self, _session_id: &str) {}
    fn on_before_llm_call(&self, _message_count: usize) {}
    fn on_after_llm_call(&self, _response: &ChatResponse) {}
    fn on_before_tool_exec(&self, _tool: &ToolSchema) {}
    fn on_after_tool_exec(&self, _tool: &ToolSchema, _result: &str) {}
    fn on_agent_end(&self, _session_id: &str, _response: &str) {}
}

#[derive(Debug, Default)]
pub struct NoopHooks;

impl AgentHooks for NoopHooks {}

#[derive(Debug, Clone)]
pub struct TraceHooks {
    logs: TraceLogService,
}

impl TraceHooks {
    pub fn new(logs: TraceLogService) -> Self {
        Self { logs }
    }

    fn record(&self, event_type: &str, payload: serde_json::Value) {
        let _ = self.logs.record(
            "agent_hook",
            serde_json::json!({
                "eventType": event_type,
                "payload": payload,
            }),
        );
    }
}

impl AgentHooks for TraceHooks {
    fn on_agent_start(&self, session_id: &str) {
        self.record(
            "agent_start",
            serde_json::json!({ "sessionId": session_id }),
        );
    }

    fn on_before_llm_call(&self, message_count: usize) {
        self.record(
            "before_llm_call",
            serde_json::json!({ "messageCount": message_count }),
        );
    }

    fn on_after_llm_call(&self, response: &ChatResponse) {
        self.record(
            "after_llm_call",
            serde_json::json!({
                "model": response.model,
                "finishReason": response.finish_reason,
                "inputTokens": response.usage.input_tokens,
                "outputTokens": response.usage.output_tokens,
            }),
        );
    }

    fn on_before_tool_exec(&self, tool: &ToolSchema) {
        self.record(
            "before_tool_exec",
            serde_json::json!({ "toolName": tool.name }),
        );
    }

    fn on_after_tool_exec(&self, tool: &ToolSchema, result: &str) {
        self.record(
            "after_tool_exec",
            serde_json::json!({
                "toolName": tool.name,
                "outputPreview": preview(result),
            }),
        );
    }

    fn on_agent_end(&self, session_id: &str, response: &str) {
        self.record(
            "agent_end",
            serde_json::json!({
                "sessionId": session_id,
                "responsePreview": preview(response),
            }),
        );
    }
}

#[derive(Debug, Clone, Default)]
pub struct RecordingHooks {
    events: Arc<Mutex<Vec<AgentHookEvent>>>,
}

impl RecordingHooks {
    pub fn events(&self) -> Vec<AgentHookEvent> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }

    fn push(&self, event: AgentHookEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

impl AgentHooks for RecordingHooks {
    fn on_agent_start(&self, session_id: &str) {
        self.push(AgentHookEvent::AgentStart {
            session_id: session_id.to_string(),
        });
    }

    fn on_before_llm_call(&self, message_count: usize) {
        self.push(AgentHookEvent::BeforeLlmCall { message_count });
    }

    fn on_after_llm_call(&self, response: &ChatResponse) {
        self.push(AgentHookEvent::AfterLlmCall {
            model: response.model.clone(),
            output_tokens: response.usage.output_tokens,
        });
    }

    fn on_before_tool_exec(&self, tool: &ToolSchema) {
        self.push(AgentHookEvent::BeforeToolExec {
            tool_name: tool.name.clone(),
        });
    }

    fn on_after_tool_exec(&self, tool: &ToolSchema, result: &str) {
        self.push(AgentHookEvent::AfterToolExec {
            tool_name: tool.name.clone(),
            output_preview: preview(result),
        });
    }

    fn on_agent_end(&self, session_id: &str, response: &str) {
        self.push(AgentHookEvent::AgentEnd {
            session_id: session_id.to_string(),
            response_preview: preview(response),
        });
    }
}

fn preview(value: &str) -> String {
    value.chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_hook_events() {
        let hooks = RecordingHooks::default();
        hooks.on_agent_start("session-1");
        hooks.on_agent_end("session-1", "done");
        assert_eq!(hooks.events().len(), 2);
    }
}
