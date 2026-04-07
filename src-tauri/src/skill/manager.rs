use std::collections::{BTreeMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension};

use crate::db::sqlite::Database;
use crate::error::AppResult;

use super::loader::{load_skill_with_default, SkillRecord};

const MAX_CANDIDATES_PER_ROOT: usize = 300;
const MAX_SKILLS_PER_SYNC: usize = 200;
const MAX_SKILL_FILE_BYTES: u64 = 256 * 1024;

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
            self.log_event(
                "sync_dir_created",
                serde_json::json!({ "root": root.display().to_string() }),
            );
        }

        let discover_root = resolve_nested_skills_root(root, MAX_CANDIDATES_PER_ROOT);
        let root_real = canonical_or_self(&discover_root);

        let mut candidates =
            collect_candidate_skill_files(&discover_root, MAX_CANDIDATES_PER_ROOT)?;
        let mut truncated = false;
        if candidates.len() > MAX_SKILLS_PER_SYNC {
            candidates.truncate(MAX_SKILLS_PER_SYNC);
            truncated = true;
        }

        self.log_event(
            "sync_started",
            serde_json::json!({
                "root": root.display().to_string(),
                "discoverRoot": discover_root.display().to_string(),
                "defaultEnabled": default_enabled,
                "candidates": candidates.len(),
                "truncated": truncated,
            }),
        );

        let mut synced = Vec::new();
        let mut skipped = 0usize;
        for candidate in candidates {
            match self.sync_single_skill_file(&candidate, &root_real, default_enabled)? {
                Some(skill) => synced.push(skill),
                None => skipped += 1,
            }
        }

        self.log_event(
            "sync_completed",
            serde_json::json!({
                "root": root.display().to_string(),
                "discoverRoot": discover_root.display().to_string(),
                "synced": synced.len(),
                "skipped": skipped,
            }),
        );

        Ok(synced)
    }

    pub fn sync_from_dirs(&self, roots: &[SkillSyncSource<'_>]) -> AppResult<Vec<SkillRecord>> {
        let mut merged = BTreeMap::new();
        for source in roots {
            for skill in self.sync_from_dir(source.root, source.default_enabled)? {
                merged.insert(skill.name.clone(), skill);
            }
        }

        let merged_skills = merged.into_values().collect::<Vec<_>>();
        self.log_event(
            "sync_from_dirs_completed",
            serde_json::json!({
                "sources": roots.len(),
                "merged": merged_skills.len(),
            }),
        );
        Ok(merged_skills)
    }

    pub fn ensure_default_skill_files(&self, root: &Path) -> AppResult<()> {
        if !root.exists() {
            std::fs::create_dir_all(root)?;
        }

        let bundled_root = resolve_bundled_skills_dir();
        let copied_from_bundle = if let Some(path) = bundled_root.as_ref() {
            copy_bundled_skills_if_missing(path, root)?
        } else {
            0
        };

        let mut fallback_created = 0usize;
        for (name, content) in default_skill_files() {
            let skill_dir = root.join(name);
            std::fs::create_dir_all(&skill_dir)?;
            let skill_file = skill_dir.join("SKILL.md");
            if !skill_file.exists() {
                std::fs::write(skill_file, content)?;
                fallback_created += 1;
            }
        }

        self.log_event(
            "ensure_default_skill_files",
            serde_json::json!({
                "root": root.display().to_string(),
                "bundledRoot": bundled_root.map(|path| path.display().to_string()),
                "copiedFromBundle": copied_from_bundle,
                "fallbackCreated": fallback_created,
            }),
        );

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
            let tools_json: String = row.get(5)?;
            let mcp_servers_json: String = row.get(6)?;
            Ok(SkillRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                path: row.get::<_, String>(3)?.into(),
                instructions: row.get(4)?,
                tools: parse_json_string_array(&tools_json),
                mcp_servers: parse_json_string_array(&mcp_servers_json),
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
        let updated = connection.execute(
            "UPDATE skills SET enabled = ?2, updated_at = ?3 WHERE name = ?1",
            params![
                name,
                if enabled { 1 } else { 0 },
                chrono::Utc::now().to_rfc3339()
            ],
        )?;

        self.log_event(
            "skill_toggled",
            serde_json::json!({
                "name": name,
                "enabled": enabled,
                "rowsAffected": updated,
            }),
        );
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

        let prompt = format_skills_for_prompt(&skills);
        self.log_event(
            "active_instructions_rendered",
            serde_json::json!({
                "enabledSkills": skills.len(),
                "promptChars": prompt.chars().count(),
            }),
        );
        Ok(prompt)
    }

    fn sync_single_skill_file(
        &self,
        file_path: &Path,
        root_real: &Path,
        default_enabled: bool,
    ) -> AppResult<Option<SkillRecord>> {
        if path_is_symlink(file_path) {
            self.log_event(
                "skill_skipped_symlink",
                serde_json::json!({ "path": file_path.display().to_string() }),
            );
            return Ok(None);
        }

        let Some(real_file_path) = canonicalized_contained(file_path, root_real) else {
            self.log_event(
                "skill_skipped_outside_root",
                serde_json::json!({
                    "path": file_path.display().to_string(),
                    "root": root_real.display().to_string(),
                }),
            );
            return Ok(None);
        };

        let metadata = match std::fs::metadata(&real_file_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                self.log_event(
                    "skill_skipped_stat_failed",
                    serde_json::json!({
                        "path": real_file_path.display().to_string(),
                        "error": error.to_string(),
                    }),
                );
                return Ok(None);
            }
        };

        if metadata.len() > MAX_SKILL_FILE_BYTES {
            self.log_event(
                "skill_skipped_oversized",
                serde_json::json!({
                    "path": real_file_path.display().to_string(),
                    "bytes": metadata.len(),
                    "maxBytes": MAX_SKILL_FILE_BYTES,
                }),
            );
            return Ok(None);
        }

        let mut skill = match load_skill_with_default(&real_file_path, default_enabled) {
            Ok(skill) => skill,
            Err(error) => {
                self.log_event(
                    "skill_parse_failed",
                    serde_json::json!({
                        "path": real_file_path.display().to_string(),
                        "error": error.message,
                    }),
                );
                return Ok(None);
            }
        };

        if let Some(existing) = self.get_by_name(&skill.name)? {
            skill.enabled = existing.enabled;
            skill.id = existing.id;
        }

        self.upsert(&skill)?;
        self.log_event(
            "skill_upserted",
            serde_json::json!({
                "name": skill.name.clone(),
                "path": skill.path.display().to_string(),
                "enabled": skill.enabled,
                "tools": skill.tools.clone(),
                "mcpServers": skill.mcp_servers.clone(),
            }),
        );
        Ok(Some(skill))
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
                &skill.id,
                &skill.name,
                &skill.description,
                skill.path.display().to_string(),
                &skill.instructions,
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
                let tools_json: String = row.get(5)?;
                let mcp_servers_json: String = row.get(6)?;
                Ok(SkillRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    path: row.get::<_, String>(3)?.into(),
                    instructions: row.get(4)?,
                    tools: parse_json_string_array(&tools_json),
                    mcp_servers: parse_json_string_array(&mcp_servers_json),
                    enabled: row.get::<_, i64>(7)? != 0,
                })
            })
            .optional()?;
        Ok(skill)
    }

    fn log_event(&self, event: &str, payload: serde_json::Value) {
        let value = serde_json::json!({
            "event": event,
            "payload": payload,
        });

        let Ok(payload_json) = serde_json::to_string(&value) else {
            return;
        };

        let _ = self.db.append_log(
            "skill_manager",
            &payload_json,
            &chrono::Utc::now().to_rfc3339(),
        );
    }
}

fn parse_json_string_array(text: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(text).unwrap_or_default()
}

fn collect_candidate_skill_files(root: &Path, scan_limit: usize) -> AppResult<Vec<PathBuf>> {
    let mut candidates = Vec::new();
    if let Some(root_skill_file) = normalize_skill_markdown_path(root) {
        candidates.push(root_skill_file);
    }

    let mut entries = std::fs::read_dir(root)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| !is_ignored_entry_name(&entry.file_name()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries.into_iter().take(scan_limit) {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if !file_type.is_dir() && !file_type.is_symlink() {
            continue;
        }

        if let Some(file_path) = normalize_skill_markdown_path(&entry.path()) {
            candidates.push(file_path);
        }
    }

    Ok(candidates)
}

fn resolve_nested_skills_root(root: &Path, scan_limit: usize) -> PathBuf {
    let nested = root.join("skills");
    if !nested.is_dir() {
        return root.to_path_buf();
    }

    let Ok(entries) = std::fs::read_dir(&nested) else {
        return root.to_path_buf();
    };

    for entry in entries.filter_map(|entry| entry.ok()).take(scan_limit) {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let skill_dir = entry.path();
        if skill_dir.join("SKILL.md").is_file() || skill_dir.join("skill.md").is_file() {
            return nested;
        }
    }

    root.to_path_buf()
}

fn normalize_skill_markdown_path(skill_dir: &Path) -> Option<PathBuf> {
    if !skill_dir.is_dir() {
        return None;
    }

    let canonical = skill_dir.join("SKILL.md");
    if canonical.is_file() {
        return Some(canonical);
    }

    let legacy = skill_dir.join("skill.md");
    if !legacy.is_file() {
        return None;
    }

    let temp = skill_dir.join(format!(".{}.tmp_skill_md", uuid::Uuid::new_v4()));
    if std::fs::rename(&legacy, &temp).is_ok() {
        if std::fs::rename(&temp, &canonical).is_ok() {
            return Some(canonical);
        }
        let _ = std::fs::rename(&temp, &legacy);
    }

    Some(legacy)
}

fn copy_bundled_skills_if_missing(source_root: &Path, target_root: &Path) -> AppResult<usize> {
    let source_real = canonical_or_self(source_root);
    let target_real = canonical_or_self(target_root);
    if source_real == target_real {
        return Ok(0);
    }

    let mut entries = std::fs::read_dir(source_root)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| !is_ignored_entry_name(&entry.file_name()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    let mut copied = 0usize;
    for entry in entries {
        let source_skill_dir = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        if !source_skill_dir.join("SKILL.md").is_file()
            && !source_skill_dir.join("skill.md").is_file()
        {
            continue;
        }

        let target_skill_dir = target_root.join(entry.file_name());
        if target_skill_dir.join("SKILL.md").is_file()
            || target_skill_dir.join("skill.md").is_file()
        {
            continue;
        }

        copy_dir_recursive(&source_skill_dir, &target_skill_dir)?;
        let _ = normalize_skill_markdown_path(&target_skill_dir);
        copied += 1;
    }

    Ok(copied)
}

fn copy_dir_recursive(source: &Path, target: &Path) -> AppResult<()> {
    std::fs::create_dir_all(target)?;

    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
            continue;
        }
        if file_type.is_file() {
            std::fs::copy(&source_path, &target_path)?;
        }
    }

    Ok(())
}

fn resolve_bundled_skills_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = std::env::var("CODEFORGE_BUNDLED_SKILLS_DIR")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        candidates.push(path);
    }

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("skills"));
    }

    if let Ok(exe_path) = std::env::current_exe() {
        let mut parent = exe_path.parent().map(PathBuf::from);
        for _ in 0..3 {
            let Some(current_parent) = parent else {
                break;
            };
            candidates.push(current_parent.join("skills"));
            parent = current_parent.parent().map(PathBuf::from);
        }
    }

    let mut seen = HashSet::new();
    for candidate in candidates {
        let normalized = candidate.to_string_lossy().to_string();
        if !seen.insert(normalized) {
            continue;
        }

        if looks_like_skills_dir(&candidate) {
            return Some(candidate);
        }
    }

    None
}

fn looks_like_skills_dir(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }

    if dir.join("SKILL.md").is_file() || dir.join("skill.md").is_file() {
        return true;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.filter_map(|entry| entry.ok()) {
        if is_ignored_entry_name(&entry.file_name()) {
            continue;
        }

        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
        {
            return true;
        }

        if file_type.is_dir()
            && (path.join("SKILL.md").is_file() || path.join("skill.md").is_file())
        {
            return true;
        }
    }

    false
}

fn format_skills_for_prompt(skills: &[SkillRecord]) -> String {
    let mut lines = vec![
        "".to_string(),
        "".to_string(),
        "The following skills provide specialized instructions for specific tasks.".to_string(),
        "Use the read_file tool to load a skill's file when the task matches its description."
            .to_string(),
        "When a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md) and use that absolute path in tool commands."
            .to_string(),
        "".to_string(),
        "<available_skills>".to_string(),
    ];

    for skill in skills {
        let name = escape_xml(&sanitize_prompt_text(&skill.name));
        let description = if skill.description.trim().is_empty() {
            "No description".to_string()
        } else {
            sanitize_prompt_text(&skill.description)
        };
        let location = compact_home_path(&skill.path);

        lines.push("  <skill>".to_string());
        lines.push(format!("    <name>{}</name>", name));
        lines.push(format!(
            "    <description>{}</description>",
            escape_xml(&description)
        ));
        lines.push(format!(
            "    <location>{}</location>",
            escape_xml(&location)
        ));
        lines.push("  </skill>".to_string());
    }

    lines.push("</available_skills>".to_string());
    lines.join("\n")
}

fn compact_home_path(path: &Path) -> String {
    let normalized_path = path.display().to_string().replace('\\', "/");
    let Some(home) = home_dir() else {
        return normalized_path;
    };

    let normalized_home = home.display().to_string().replace('\\', "/");
    let home_with_sep = if normalized_home.ends_with('/') {
        normalized_home
    } else {
        format!("{normalized_home}/")
    };

    if normalized_path.starts_with(&home_with_sep) {
        format!("~/{}", &normalized_path[home_with_sep.len()..])
    } else {
        normalized_path
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn sanitize_prompt_text(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .collect::<String>()
        .replace('`', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn path_is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn canonicalized_contained(path: &Path, root_real: &Path) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;
    if canonical.starts_with(root_real) {
        Some(canonical)
    } else {
        None
    }
}

fn canonical_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn is_ignored_entry_name(name: &OsStr) -> bool {
    let as_text = name.to_string_lossy();
    as_text.starts_with('.') || as_text.eq_ignore_ascii_case("node_modules")
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
        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("demo-skill"));
        assert!(prompt.contains("demo description"));
        assert!(!prompt.contains("VERY LONG BODY"));
    }

    #[test]
    fn sync_from_dir_normalizes_legacy_skill_file_name() {
        let db_path = std::env::temp_dir().join(format!(
            "codeforge-skill-manager-legacy-{}.db",
            uuid::Uuid::new_v4()
        ));
        let db = Database::new(&db_path).expect("db should initialize");
        let manager = SkillManager::new(db);
        let root = std::env::temp_dir().join(format!(
            "codeforge-skill-legacy-root-{}",
            uuid::Uuid::new_v4()
        ));
        let skill_dir = root.join("legacy-skill");
        std::fs::create_dir_all(&skill_dir).expect("skill dir should exist");
        std::fs::write(
            skill_dir.join("skill.md"),
            "---\nname: legacy-skill\ndescription: from legacy file\n---\nBody",
        )
        .expect("legacy skill file should exist");

        let synced = manager
            .sync_from_dir(&root, true)
            .expect("skills should sync");

        assert_eq!(synced.len(), 1);
        assert!(skill_dir.join("SKILL.md").exists());
    }

    #[test]
    fn sync_skips_oversized_skill_file() {
        let db_path = std::env::temp_dir().join(format!(
            "codeforge-skill-manager-oversized-{}.db",
            uuid::Uuid::new_v4()
        ));
        let db = Database::new(&db_path).expect("db should initialize");
        let manager = SkillManager::new(db.clone());
        let root = std::env::temp_dir().join(format!(
            "codeforge-skill-oversized-root-{}",
            uuid::Uuid::new_v4()
        ));
        let skill_dir = root.join("oversized-skill");
        std::fs::create_dir_all(&skill_dir).expect("skill dir should exist");
        let oversized_body = "A".repeat((MAX_SKILL_FILE_BYTES + 4) as usize);
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                "---\nname: oversized-skill\ndescription: too large\n---\n{}",
                oversized_body
            ),
        )
        .expect("oversized skill file should exist");

        let synced = manager
            .sync_from_dir(&root, true)
            .expect("sync should not fail");
        assert!(synced.is_empty());

        let connection = db.connection().expect("db connection should open");
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM logs WHERE kind = 'skill_manager'",
                [],
                |row| row.get(0),
            )
            .expect("log count should query");
        assert!(count > 0);
    }
}
