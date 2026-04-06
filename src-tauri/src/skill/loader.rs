use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub instructions: String,
    pub tools: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
struct SkillFrontmatter {
    name: String,
    description: String,
    tools: Vec<String>,
    mcp_servers: Vec<String>,
}

pub fn load_skill(path: &Path) -> AppResult<SkillRecord> {
    load_skill_with_default(path, true)
}

pub fn load_skill_with_default(path: &Path, default_enabled: bool) -> AppResult<SkillRecord> {
    let text = std::fs::read_to_string(path)?;
    let (frontmatter, instructions) = split_frontmatter(&text)?;
    if frontmatter.name.trim().is_empty() {
        return Err(AppError::new(format!(
            "Skill 文件缺少 name: {}",
            path.display()
        )));
    }

    Ok(SkillRecord {
        id: uuid::Uuid::new_v4().to_string(),
        name: frontmatter.name.trim().to_string(),
        description: frontmatter.description.trim().to_string(),
        path: path.to_path_buf(),
        instructions: instructions.trim().to_string(),
        tools: frontmatter.tools,
        mcp_servers: frontmatter.mcp_servers,
        enabled: default_enabled,
    })
}

fn split_frontmatter(text: &str) -> AppResult<(SkillFrontmatter, String)> {
    if !text.starts_with("---") {
        return Ok((SkillFrontmatter::default(), text.to_string()));
    }

    let mut parts = text.splitn(3, "---");
    let _ = parts.next();
    let yaml = parts
        .next()
        .ok_or_else(|| AppError::new("Skill frontmatter 不完整"))?;
    let instructions = parts.next().unwrap_or_default().to_string();
    let frontmatter = serde_yaml::from_str::<SkillFrontmatter>(yaml)?;
    Ok((frontmatter, instructions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skill_markdown() {
        let dir = std::env::temp_dir().join(format!("codeforge-skill-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should exist");
        let file = dir.join("SKILL.md");
        std::fs::write(
            &file,
            "---\nname: code-review\ndescription: review code\ntools:\n  - read_file\n---\nBe careful.",
        )
        .expect("skill file should exist");

        let skill = load_skill(&file).expect("skill should parse");
        assert_eq!(skill.name, "code-review");
        assert_eq!(skill.tools, vec!["read_file"]);
    }
}
