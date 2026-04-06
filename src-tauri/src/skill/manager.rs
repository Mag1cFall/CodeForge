use std::path::Path;

use rusqlite::{params, OptionalExtension};

use crate::db::sqlite::Database;
use crate::error::AppResult;

use super::loader::{load_skill, SkillRecord};

#[derive(Debug, Clone)]
pub struct SkillManager {
    db: Database,
}

impl SkillManager {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn sync_from_dir(&self, root: &Path) -> AppResult<Vec<SkillRecord>> {
        if !root.exists() {
            std::fs::create_dir_all(root)?;
        }

        let mut skills = Vec::new();
        for entry in walkdir::WalkDir::new(root) {
            let entry = entry.map_err(|error| crate::error::AppError::new(error.to_string()))?;
            if !entry.file_type().is_file() || entry.file_name() != "SKILL.md" {
                continue;
            }

            let mut skill = load_skill(entry.path())?;
            if let Some(existing) = self.get_by_name(&skill.name)? {
                skill.enabled = existing.enabled;
                skill.id = existing.id;
            }
            self.upsert(&skill)?;
            skills.push(skill);
        }
        Ok(skills)
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
        Ok(self
            .list()?
            .into_iter()
            .filter(|skill| skill.enabled && !skill.instructions.trim().is_empty())
            .map(|skill| format!("[{}]\n{}", skill.name, skill.instructions))
            .collect::<Vec<_>>()
            .join("\n\n"))
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
