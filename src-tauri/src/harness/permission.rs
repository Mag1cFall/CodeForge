use std::sync::Arc;

use crate::error::AppResult;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionPolicy {
    AlwaysAllow,
    AskUser,
    AlwaysDeny,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub id: String,
    pub tool_name: String,
    pub args: serde_json::Value,
    pub risk_level: RiskLevel,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct PermissionManager {
    rules: Arc<Vec<PermissionRule>>,
}

#[derive(Debug, Clone)]
struct PermissionRule {
    pattern: &'static str,
    policy: PermissionPolicy,
    risk: RiskLevel,
    description: &'static str,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            rules: Arc::new(default_rules()),
        }
    }

    pub fn classify(&self, tool_name: &str) -> (PermissionPolicy, RiskLevel, String) {
        let normalized = tool_name.trim().to_ascii_lowercase();
        let mut matched: Option<&PermissionRule> = None;
        for rule in self.rules.iter() {
            if wildcard_match(rule.pattern, &normalized) {
                matched = Some(rule);
            }
        }

        let selected = matched.unwrap_or(&PermissionRule {
            pattern: "*",
            policy: PermissionPolicy::AskUser,
            risk: RiskLevel::Low,
            description: "未知工具默认要求确认。",
        });

        log_permission_event(
            "classify",
            serde_json::json!({
                "tool": normalized,
                "pattern": selected.pattern,
                "policy": selected.policy,
                "risk": selected.risk,
            }),
        );

        (
            selected.policy,
            selected.risk,
            selected.description.to_string(),
        )
    }

    pub fn ensure_allowed(
        &self,
        tool_name: &str,
    ) -> AppResult<(PermissionPolicy, RiskLevel, String)> {
        Ok(self.classify(tool_name))
    }
}

fn default_rules() -> Vec<PermissionRule> {
    vec![
        PermissionRule {
            pattern: "*",
            policy: PermissionPolicy::AskUser,
            risk: RiskLevel::Low,
            description: "未知工具默认要求确认。",
        },
        PermissionRule {
            pattern: "read_file",
            policy: PermissionPolicy::AlwaysAllow,
            risk: RiskLevel::Low,
            description: "读取文件属于只读操作，可直接执行。",
        },
        PermissionRule {
            pattern: "list_directory",
            policy: PermissionPolicy::AlwaysAllow,
            risk: RiskLevel::Low,
            description: "目录枚举属于只读操作，可直接执行。",
        },
        PermissionRule {
            pattern: "search_code",
            policy: PermissionPolicy::AlwaysAllow,
            risk: RiskLevel::Low,
            description: "代码检索属于只读操作，可直接执行。",
        },
        PermissionRule {
            pattern: "grep_pattern",
            policy: PermissionPolicy::AlwaysAllow,
            risk: RiskLevel::Low,
            description: "正则检索属于只读操作，可直接执行。",
        },
        PermissionRule {
            pattern: "analyze_ast",
            policy: PermissionPolicy::AlwaysAllow,
            risk: RiskLevel::Low,
            description: "语法分析属于只读操作，可直接执行。",
        },
        PermissionRule {
            pattern: "check_complexity",
            policy: PermissionPolicy::AlwaysAllow,
            risk: RiskLevel::Low,
            description: "复杂度检查属于只读操作，可直接执行。",
        },
        PermissionRule {
            pattern: "find_code_smells",
            policy: PermissionPolicy::AlwaysAllow,
            risk: RiskLevel::Low,
            description: "代码异味扫描属于只读操作，可直接执行。",
        },
        PermissionRule {
            pattern: "suggest_refactor",
            policy: PermissionPolicy::AlwaysAllow,
            risk: RiskLevel::Low,
            description: "重构建议属于只读操作，可直接执行。",
        },
        PermissionRule {
            pattern: "*write*",
            policy: PermissionPolicy::AskUser,
            risk: RiskLevel::Medium,
            description: "写入操作会修改项目内容，需要显式确认。",
        },
        PermissionRule {
            pattern: "*patch*",
            policy: PermissionPolicy::AskUser,
            risk: RiskLevel::Medium,
            description: "补丁操作会修改项目内容，需要显式确认。",
        },
        PermissionRule {
            pattern: "*edit*",
            policy: PermissionPolicy::AskUser,
            risk: RiskLevel::Medium,
            description: "编辑操作会修改项目内容，需要显式确认。",
        },
        PermissionRule {
            pattern: "run_shell",
            policy: PermissionPolicy::AskUser,
            risk: RiskLevel::High,
            description: "Shell 命令会直接作用于运行环境，需要显式确认。",
        },
        PermissionRule {
            pattern: "run_tests",
            policy: PermissionPolicy::AskUser,
            risk: RiskLevel::High,
            description: "测试命令可执行任意脚本，需要显式确认。",
        },
        PermissionRule {
            pattern: "*shell*",
            policy: PermissionPolicy::AskUser,
            risk: RiskLevel::High,
            description: "Shell 类操作风险较高，需要显式确认。",
        },
        PermissionRule {
            pattern: "*delete*",
            policy: PermissionPolicy::AlwaysDeny,
            risk: RiskLevel::High,
            description: "删除类操作默认被 Harness 拒绝。",
        },
    ]
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    let value = value.trim().to_ascii_lowercase();

    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == value;
    }

    let mut search_start = 0usize;
    let parts = pattern
        .split('*')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return true;
    }

    for (index, part) in parts.iter().enumerate() {
        if index == 0 && !pattern.starts_with('*') {
            if !value[search_start..].starts_with(part) {
                return false;
            }
            search_start += part.len();
            continue;
        }

        let Some(relative_index) = value[search_start..].find(part) else {
            return false;
        };
        search_start += relative_index + part.len();
    }

    if !pattern.ends_with('*') {
        if let Some(last) = parts.last() {
            return value.ends_with(last);
        }
    }

    true
}

fn log_permission_event(event: &str, payload: serde_json::Value) {
    eprintln!(
        "{}",
        serde_json::json!({
            "component": "harness.permission",
            "event": event,
            "payload": payload,
        })
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_run_shell_as_high_risk() {
        let manager = PermissionManager::new();
        let (policy, risk, _) = manager.classify("run_shell");
        assert_eq!(policy, PermissionPolicy::AskUser);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn classifies_delete_as_denied() {
        let manager = PermissionManager::new();
        let (policy, risk, _) = manager.classify("delete_file");
        assert_eq!(policy, PermissionPolicy::AlwaysDeny);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn classifies_read_as_allow() {
        let manager = PermissionManager::new();
        let (policy, risk, _) = manager.classify("read_file");
        assert_eq!(policy, PermissionPolicy::AlwaysAllow);
        assert_eq!(risk, RiskLevel::Low);
    }
}
