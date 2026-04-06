use crate::error::AppResult;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionPolicy {
    AlwaysAllow,
    AskUser,
    AlwaysDeny,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
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
pub struct PermissionManager;

impl PermissionManager {
    pub fn new() -> Self {
        Self
    }

    pub fn classify(&self, tool_name: &str) -> (PermissionPolicy, RiskLevel, String) {
        match tool_name {
            "run_shell" => (
                PermissionPolicy::AskUser,
                RiskLevel::High,
                "Shell 命令会直接作用于运行环境，需要显式确认。".into(),
            ),
            "write_file" | "apply_patch" => (
                PermissionPolicy::AskUser,
                RiskLevel::Medium,
                "文件修改会改变项目内容，需要显式确认。".into(),
            ),
            "delete_file" => (
                PermissionPolicy::AlwaysDeny,
                RiskLevel::High,
                "删除文件默认被 Harness 拒绝。".into(),
            ),
            _ => (
                PermissionPolicy::AlwaysAllow,
                RiskLevel::Low,
                "只读或分析型操作可直接执行。".into(),
            ),
        }
    }

    pub fn ensure_allowed(
        &self,
        tool_name: &str,
    ) -> AppResult<(PermissionPolicy, RiskLevel, String)> {
        Ok(self.classify(tool_name))
    }
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
}
