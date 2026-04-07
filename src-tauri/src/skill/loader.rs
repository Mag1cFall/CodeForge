use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use serde_yaml::{Mapping, Value};

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

#[derive(Debug, Clone, Default)]
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
    let (frontmatter, instructions) = split_frontmatter(&text);

    let name = derive_skill_name(path, &frontmatter.name);
    if name.is_empty() {
        return Err(AppError::new(format!(
            "Skill 文件缺少 name: {}",
            path.display()
        )));
    }

    let description = if frontmatter.description.trim().is_empty() {
        derive_description(&instructions)
    } else {
        frontmatter.description.trim().to_string()
    };

    Ok(SkillRecord {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        description,
        path: path.to_path_buf(),
        instructions: instructions.trim().to_string(),
        tools: frontmatter.tools,
        mcp_servers: frontmatter.mcp_servers,
        enabled: default_enabled,
    })
}

fn split_frontmatter(text: &str) -> (SkillFrontmatter, String) {
    let normalized = normalize_line_endings(strip_utf8_bom(text));
    let mut lines = normalized.lines();
    let Some(first_line) = lines.next() else {
        return (SkillFrontmatter::default(), String::new());
    };

    if first_line.trim() != "---" {
        return (SkillFrontmatter::default(), normalized.to_string());
    }

    let mut yaml_lines = Vec::new();
    let mut body_lines = Vec::new();
    let mut in_body = false;

    for line in lines {
        if !in_body && line.trim() == "---" {
            in_body = true;
            continue;
        }

        if in_body {
            body_lines.push(line);
        } else {
            yaml_lines.push(line);
        }
    }

    if !in_body {
        return (SkillFrontmatter::default(), normalized.to_string());
    }

    let yaml = yaml_lines.join("\n");
    let instructions = body_lines.join("\n");
    (parse_frontmatter_block(&yaml), instructions)
}

fn parse_frontmatter_block(block: &str) -> SkillFrontmatter {
    match serde_yaml::from_str::<Value>(block) {
        Ok(Value::Mapping(map)) => parse_frontmatter_mapping(&map),
        _ => parse_line_frontmatter(block),
    }
}

fn parse_frontmatter_mapping(map: &Mapping) -> SkillFrontmatter {
    SkillFrontmatter {
        name: mapping_string(map, "name"),
        description: mapping_string(map, "description"),
        tools: mapping_string_list(map, "tools"),
        mcp_servers: mapping_string_list_multi(map, &["mcp_servers", "mcp-servers"]),
    }
}

fn parse_line_frontmatter(block: &str) -> SkillFrontmatter {
    let mut frontmatter = SkillFrontmatter::default();
    for line in block.lines() {
        let Some((raw_key, raw_value)) = line.split_once(':') else {
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase();
        let value = raw_value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        if value.is_empty() {
            continue;
        }

        match key.as_str() {
            "name" => frontmatter.name = value,
            "description" => frontmatter.description = value,
            "tools" => frontmatter.tools.push(value),
            "mcp_servers" => frontmatter.mcp_servers.push(value),
            _ => {}
        }
    }
    frontmatter
}

fn mapping_string(map: &Mapping, key: &str) -> String {
    let Some(value) = mapping_value(map, &[key]) else {
        return String::new();
    };

    yaml_scalar_to_string(value)
        .map(|text| text.trim().to_string())
        .unwrap_or_default()
}

fn mapping_string_list(map: &Mapping, key: &str) -> Vec<String> {
    mapping_string_list_multi(map, &[key])
}

fn mapping_string_list_multi(map: &Mapping, keys: &[&str]) -> Vec<String> {
    let Some(value) = mapping_value(map, keys) else {
        return Vec::new();
    };

    match value {
        Value::Sequence(items) => items
            .iter()
            .filter_map(yaml_scalar_to_string)
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
        _ => yaml_scalar_to_string(value)
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .map(|item| vec![item])
            .unwrap_or_default(),
    }
}

fn mapping_value<'a>(map: &'a Mapping, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        if let Some(value) = map.get(&Value::String((*key).to_string())) {
            return Some(value);
        }
    }
    None
}

fn yaml_scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn derive_skill_name(path: &Path, frontmatter_name: &str) -> String {
    let trimmed = frontmatter_name.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    if let Some(name) = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return name.to_string();
    }

    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn derive_description(markdown_body: &str) -> String {
    let lines: Vec<&str> = markdown_body.lines().collect();
    let heading_index = lines.iter().position(|line| {
        matches!(
            line.trim().to_ascii_lowercase().as_str(),
            "## 描述" | "## description"
        )
    });

    if let Some(index) = heading_index {
        for line in lines.iter().skip(index + 1) {
            let content = line.trim();
            if content.is_empty() {
                continue;
            }
            if content.starts_with('#') {
                break;
            }
            return content.to_string();
        }
    }

    for line in lines {
        let content = line.trim();
        if content.is_empty() || content.starts_with('#') {
            continue;
        }
        return content.to_string();
    }

    String::new()
}

fn strip_utf8_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
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

    #[test]
    fn parses_without_frontmatter_using_dir_name() {
        let root = std::env::temp_dir().join(format!(
            "codeforge-skill-no-frontmatter-{}",
            uuid::Uuid::new_v4()
        ));
        let skill_dir = root.join("demo-no-frontmatter");
        std::fs::create_dir_all(&skill_dir).expect("skill dir should exist");
        let file = skill_dir.join("SKILL.md");
        std::fs::write(&file, "# Demo Skill\n\nThis skill has no frontmatter.")
            .expect("skill file should exist");

        let skill = load_skill(&file).expect("skill should parse");
        assert_eq!(skill.name, "demo-no-frontmatter");
        assert_eq!(skill.description, "This skill has no frontmatter.");
    }

    #[test]
    fn derives_description_from_description_heading() {
        let root = std::env::temp_dir().join(format!(
            "codeforge-skill-description-heading-{}",
            uuid::Uuid::new_v4()
        ));
        let skill_dir = root.join("desc-heading");
        std::fs::create_dir_all(&skill_dir).expect("skill dir should exist");
        let file = skill_dir.join("SKILL.md");
        std::fs::write(
            &file,
            "---\nname: desc-heading\n---\n## 描述\n\n这是描述段落。\n\n## 其他\n内容",
        )
        .expect("skill file should exist");

        let skill = load_skill(&file).expect("skill should parse");
        assert_eq!(skill.description, "这是描述段落。");
    }

    #[test]
    fn parses_scalar_tools_and_mcp_servers() {
        let root = std::env::temp_dir().join(format!(
            "codeforge-skill-scalar-tools-{}",
            uuid::Uuid::new_v4()
        ));
        let skill_dir = root.join("scalar-tools");
        std::fs::create_dir_all(&skill_dir).expect("skill dir should exist");
        let file = skill_dir.join("SKILL.md");
        std::fs::write(
            &file,
            "---\nname: scalar-tools\ndescription: test\ntools: read_file\nmcp_servers: local-mcp\n---\nBody",
        )
        .expect("skill file should exist");

        let skill = load_skill(&file).expect("skill should parse");
        assert_eq!(skill.tools, vec!["read_file"]);
        assert_eq!(skill.mcp_servers, vec!["local-mcp"]);
    }
}
