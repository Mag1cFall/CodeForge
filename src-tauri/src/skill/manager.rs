use std::path::Path;

use rusqlite::{params, OptionalExtension};

use crate::db::sqlite::Database;
use crate::error::AppResult;

use super::loader::{load_skill_with_default, SkillRecord};

#[derive(Debug, Clone, Copy)]
pub struct SkillSyncSource<'a> {
    pub root: &'a Path,
    pub default_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct SkillManager {
    db: Database,
}

impl SkillManager {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn sync_from_dir(&self, root: &Path, default_enabled: bool) -> AppResult<Vec<SkillRecord>> {
        if !root.exists() {
            std::fs::create_dir_all(root)?;
        }

        let mut skills = Vec::new();
        for entry in walkdir::WalkDir::new(root) {
            let entry = entry.map_err(|error| crate::error::AppError::new(error.to_string()))?;
            if !entry.file_type().is_file() || entry.file_name() != "SKILL.md" {
                continue;
            }

            let mut skill = load_skill_with_default(entry.path(), default_enabled)?;
            if let Some(existing) = self.get_by_name(&skill.name)? {
                skill.enabled = existing.enabled;
                skill.id = existing.id;
            }
            self.upsert(&skill)?;
            skills.push(skill);
        }
        Ok(skills)
    }

    pub fn sync_from_dirs(&self, roots: &[SkillSyncSource<'_>]) -> AppResult<Vec<SkillRecord>> {
        let mut merged = Vec::new();
        for source in roots {
            merged.extend(self.sync_from_dir(source.root, source.default_enabled)?);
        }
        Ok(merged)
    }

    pub fn ensure_default_skill_files(&self, root: &Path) -> AppResult<()> {
        if !root.exists() {
            std::fs::create_dir_all(root)?;
        }

        for (name, content) in default_skill_files() {
            let skill_dir = root.join(name);
            std::fs::create_dir_all(&skill_dir)?;
            let skill_file = skill_dir.join("SKILL.md");
            if !skill_file.exists() {
                std::fs::write(skill_file, content)?;
            }
        }

        Ok(())
    }

    pub fn list(&self) -> AppResult<Vec<SkillRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, name, description, path, instructions, tools_json, mcp_servers_json, enabled
            FROM skills ORDER BY name ASC
            "#,
        )?;
        let rows = statement.query_map([], |row| {
            Ok(SkillRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                path: row.get::<_, String>(3)?.into(),
                instructions: row.get(4)?,
                tools: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                mcp_servers: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                enabled: row.get::<_, i64>(7)? != 0,
            })
        })?;

        let mut skills = Vec::new();
        for row in rows {
            skills.push(row?);
        }
        Ok(skills)
    }

    pub fn toggle(&self, name: &str, enabled: bool) -> AppResult<()> {
        let connection = self.db.connection()?;
        connection.execute(
            "UPDATE skills SET enabled = ?2, updated_at = ?3 WHERE name = ?1",
            params![
                name,
                if enabled { 1 } else { 0 },
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn active_instructions(&self) -> AppResult<String> {
        let skills = self
            .list()?
            .into_iter()
            .filter(|skill| skill.enabled && !skill.instructions.trim().is_empty())
            .collect::<Vec<_>>();

        if skills.is_empty() {
            return Ok(String::new());
        }

        let mut lines = vec![
            "## Skills".to_string(),
            "可用技能如下。默认只阅读名称、描述与文件路径；只有当任务明确匹配时，才使用 read_file 打开对应 SKILL.md 读取全文。".to_string(),
            "不要一次性读取所有技能文件，也不要把全部技能正文塞进上下文。".to_string(),
            String::new(),
            "### Enabled skills".to_string(),
        ];

        for skill in skills {
            lines.push(format!(
                "- {}: {}\n  File: {}",
                skill.name,
                if skill.description.trim().is_empty() {
                    "No description"
                } else {
                    skill.description.trim()
                },
                skill.path.display()
            ));
        }

        Ok(lines.join("\n"))
    }

    fn upsert(&self, skill: &SkillRecord) -> AppResult<()> {
        let connection = self.db.connection()?;
        connection.execute(
            r#"
            INSERT INTO skills (id, name, description, path, instructions, tools_json, mcp_servers_json, enabled, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(name) DO UPDATE SET
                description = excluded.description,
                path = excluded.path,
                instructions = excluded.instructions,
                tools_json = excluded.tools_json,
                mcp_servers_json = excluded.mcp_servers_json,
                enabled = excluded.enabled,
                updated_at = excluded.updated_at
            "#,
            params![
                skill.id,
                skill.name,
                skill.description,
                skill.path.display().to_string(),
                skill.instructions,
                serde_json::to_string(&skill.tools)?,
                serde_json::to_string(&skill.mcp_servers)?,
                if skill.enabled { 1 } else { 0 },
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_by_name(&self, name: &str) -> AppResult<Option<SkillRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, name, description, path, instructions, tools_json, mcp_servers_json, enabled
            FROM skills WHERE name = ?1 LIMIT 1
            "#,
        )?;
        let skill = statement
            .query_row(params![name], |row| {
                Ok(SkillRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    path: row.get::<_, String>(3)?.into(),
                    instructions: row.get(4)?,
                    tools: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                    mcp_servers: serde_json::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or_default(),
                    enabled: row.get::<_, i64>(7)? != 0,
                })
            })
            .optional()?;
        Ok(skill)
    }
}

fn default_skill_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "code-review",
            "---\nname: code-review\ndescription: 全面的代码质量审查\ntools:\n  - read_file\n  - search_code\n  - analyze_ast\n  - find_code_smells\n---\n聚焦错误处理、复杂度、命名、可维护性与潜在风险，给出可执行建议。\n",
        ),
        (
            "best-practices",
            "---\nname: best-practices\ndescription: 语言与框架最佳实践建议\ntools:\n  - read_file\n  - search_code\n  - grep_pattern\n---\n根据当前项目栈总结主流写法、约束条件与推荐模式。\n",
        ),
        (
            "security-audit",
            "---\nname: security-audit\ndescription: 常见安全问题审计\ntools:\n  - read_file\n  - grep_pattern\n  - find_code_smells\n---\n重点检查凭据泄露、危险命令、输入处理与权限边界。\n",
        ),
        (
            "refactoring",
            "---\nname: refactoring\ndescription: 结构重整与重构建议\ntools:\n  - read_file\n  - suggest_refactor\n  - apply_patch\n---\n优先控制修改范围，拆分复杂逻辑，保持行为一致与代码清晰。\n",
        ),
        (
            "documentation",
            "---\nname: documentation\ndescription: 文档与说明文字整理\ntools:\n  - read_file\n  - write_file\n---\n为功能、配置和使用方式生成简洁准确的文档内容。\n",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use crate::db::sqlite::Database;

    use super::*;

    #[test]
    fn active_instructions_use_inventory_not_full_body() {
        let db_path = std::env::temp_dir().join(format!(
            "codeforge-skill-manager-{}.db",
            uuid::Uuid::new_v4()
        ));
        let db = Database::new(&db_path).expect("db should initialize");
        let manager = SkillManager::new(db);
        let root =
            std::env::temp_dir().join(format!("codeforge-skill-root-{}", uuid::Uuid::new_v4()));
        let skill_dir = root.join("demo-skill");
        std::fs::create_dir_all(&skill_dir).expect("skill dir should exist");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: demo-skill\ndescription: demo description\n---\nTHIS IS A VERY LONG BODY THAT SHOULD NOT BE INJECTED",
        )
        .expect("skill file should exist");

        manager
            .sync_from_dir(&root, true)
            .expect("skills should sync");

        let prompt = manager
            .active_instructions()
            .expect("active skill prompt should build");
        assert!(prompt.contains("demo-skill"));
        assert!(prompt.contains("demo description"));
        assert!(!prompt.contains("VERY LONG BODY"));
    }
}
