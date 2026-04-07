use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::harness::hashline::annotate_text;

use super::emit_structured_log;

const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";
const ADD_FILE_MARKER: &str = "*** Add File: ";
const DELETE_FILE_MARKER: &str = "*** Delete File: ";
const UPDATE_FILE_MARKER: &str = "*** Update File: ";
const MOVE_TO_MARKER: &str = "*** Move to: ";
const EOF_MARKER: &str = "*** End of File";
const CHANGE_CONTEXT_MARKER: &str = "@@ ";
const EMPTY_CHANGE_CONTEXT_MARKER: &str = "@@";

#[derive(Debug, Clone)]
enum PatchHunk {
    Add {
        path: String,
        contents: String,
    },
    Delete {
        path: String,
    },
    Update {
        path: String,
        move_path: Option<String>,
        chunks: Vec<UpdateFileChunk>,
    },
}

#[derive(Debug, Clone)]
struct UpdateFileChunk {
    change_context: Option<String>,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    is_end_of_file: bool,
}

#[derive(Debug, Clone)]
struct Replacement {
    start_index: usize,
    old_len: usize,
    new_lines: Vec<String>,
}

#[derive(Debug, Default)]
struct PatchSummary {
    added: Vec<String>,
    modified: Vec<String>,
    deleted: Vec<String>,
}

pub fn read_file(path: impl AsRef<Path>) -> AppResult<String> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)?;
    if is_likely_binary(&bytes) {
        return Err(AppError::new(format!(
            "文件 {} 不是可读文本文件",
            path.display()
        )));
    }
    let content = String::from_utf8(bytes)
        .map_err(|_| AppError::new(format!("文件 {} 不是 UTF-8 文本", path.display())))?;
    emit_structured_log(
        "file_tools",
        "read_file",
        serde_json::json!({
            "path": path.display().to_string(),
            "bytes": content.len(),
            "lineCount": content.lines().count(),
        }),
    );
    Ok(annotate_text(&content))
}

pub fn list_directory(path: impl AsRef<Path>) -> AppResult<Vec<String>> {
    let path = path.as_ref();
    if !path.is_dir() {
        return Err(AppError::new(format!("路径 {} 不是目录", path.display())));
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let mut name = entry.file_name().to_string_lossy().to_string();
        if entry_path.is_dir() {
            name.push('/');
        }
        entries.push(name);
    }
    entries.sort();

    emit_structured_log(
        "file_tools",
        "list_directory",
        serde_json::json!({
            "path": path.display().to_string(),
            "count": entries.len(),
        }),
    );
    Ok(entries)
}

pub fn write_file(path: impl AsRef<Path>, content: &str) -> AppResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    emit_structured_log(
        "file_tools",
        "write_file",
        serde_json::json!({
            "path": path.display().to_string(),
            "bytes": content.len(),
            "lineCount": content.lines().count(),
        }),
    );
    Ok(())
}

pub fn apply_patch_text(path: impl AsRef<Path>, old: &str, new: &str) -> AppResult<String> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)?;
    if !content.contains(old) {
        return Err(AppError::new("补丁目标未匹配到原始内容"));
    }
    let updated = content.replacen(old, new, 1);
    std::fs::write(path, &updated)?;
    emit_structured_log(
        "file_tools",
        "apply_patch_legacy",
        serde_json::json!({
            "path": path.display().to_string(),
            "oldPreview": preview_text(old),
            "newPreview": preview_text(new),
        }),
    );
    Ok(updated)
}

pub fn apply_structured_patch(root: &Path, input: &str) -> AppResult<String> {
    let normalized_root = canonicalize_if_exists_or_normalize(root);
    let hunks = parse_patch_text(input)?;
    if hunks.is_empty() {
        return Err(AppError::new("补丁内容为空，没有可执行的修改"));
    }

    let mut summary = PatchSummary::default();
    let mut added_seen = BTreeSet::new();
    let mut modified_seen = BTreeSet::new();
    let mut deleted_seen = BTreeSet::new();

    for hunk in hunks {
        match hunk {
            PatchHunk::Add { path, contents } => {
                let target = resolve_path(Some(normalized_root.as_path()), &path)?;
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&target, contents)?;
                push_unique(
                    &mut summary.added,
                    &mut added_seen,
                    to_display_path(&target, normalized_root.as_path()),
                );
            }
            PatchHunk::Delete { path } => {
                let target = resolve_path(Some(normalized_root.as_path()), &path)?;
                if target.is_dir() {
                    std::fs::remove_dir_all(&target)?;
                } else {
                    std::fs::remove_file(&target)?;
                }
                push_unique(
                    &mut summary.deleted,
                    &mut deleted_seen,
                    to_display_path(&target, normalized_root.as_path()),
                );
            }
            PatchHunk::Update {
                path,
                move_path,
                chunks,
            } => {
                let target = resolve_path(Some(normalized_root.as_path()), &path)?;
                let applied = apply_update_hunk(&target, &chunks)?;
                if let Some(move_target_path) = move_path {
                    let move_target =
                        resolve_path(Some(normalized_root.as_path()), &move_target_path)?;
                    if let Some(parent) = move_target.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&move_target, applied)?;
                    std::fs::remove_file(&target)?;
                    push_unique(
                        &mut summary.modified,
                        &mut modified_seen,
                        to_display_path(&move_target, normalized_root.as_path()),
                    );
                } else {
                    std::fs::write(&target, applied)?;
                    push_unique(
                        &mut summary.modified,
                        &mut modified_seen,
                        to_display_path(&target, normalized_root.as_path()),
                    );
                }
            }
        }
    }

    let text = format_patch_summary(&summary);
    emit_structured_log(
        "file_tools",
        "apply_patch",
        serde_json::json!({
            "root": root.display().to_string(),
            "summary": {
                "added": summary.added,
                "modified": summary.modified,
                "deleted": summary.deleted,
            }
        }),
    );
    Ok(text)
}

pub fn resolve_path(root: Option<&Path>, input: &str) -> AppResult<PathBuf> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::new("路径不能为空"));
    }

    let input_path = PathBuf::from(trimmed);
    if let Some(root) = root {
        let normalized_root = canonicalize_if_exists_or_normalize(root);
        let resolved = if input_path.is_absolute() {
            input_path
        } else {
            normalized_root.join(input_path)
        };
        let normalized = canonicalize_if_exists_or_normalize(&resolved);
        if !is_within_root(&normalized, &normalized_root) {
            return Err(AppError::new(format!("路径超出工作区: {trimmed}")));
        }
        return Ok(normalized);
    }

    if input_path.is_absolute() {
        return Ok(canonicalize_if_exists_or_normalize(&input_path));
    }

    Err(AppError::new("相对路径缺少工作区根目录"))
}

fn parse_patch_text(input: &str) -> AppResult<Vec<PatchHunk>> {
    let normalized = input.replace("\r\n", "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Err(AppError::new("补丁内容为空"));
    }

    let raw_lines = trimmed
        .split('\n')
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let lines = normalize_patch_boundaries(raw_lines)?;
    let last_line = lines.len().saturating_sub(1);
    let mut cursor = 1usize;
    let mut hunks = Vec::new();

    while cursor < last_line {
        while cursor < last_line && lines[cursor].trim().is_empty() {
            cursor += 1;
        }
        if cursor >= last_line {
            break;
        }
        let (hunk, consumed) = parse_one_hunk(&lines[cursor..last_line], cursor + 1)?;
        hunks.push(hunk);
        cursor += consumed;
    }

    Ok(hunks)
}

fn normalize_patch_boundaries(lines: Vec<String>) -> AppResult<Vec<String>> {
    if let Err(error) = validate_patch_boundaries(&lines) {
        if lines.len() >= 4 {
            let first = lines[0].trim();
            let last = lines[lines.len() - 1].trim();
            if ["<<EOF", "<<'EOF'", "<<\"EOF\""].contains(&first) && last.ends_with("EOF") {
                let inner = lines[1..lines.len() - 1].to_vec();
                validate_patch_boundaries(&inner)?;
                return Ok(inner);
            }
        }
        return Err(error);
    }
    Ok(lines)
}

fn validate_patch_boundaries(lines: &[String]) -> AppResult<()> {
    let first = lines
        .first()
        .map(|line| line.trim())
        .ok_or_else(|| AppError::new("补丁内容为空"))?;
    let last = lines
        .last()
        .map(|line| line.trim())
        .ok_or_else(|| AppError::new("补丁内容为空"))?;

    if first != BEGIN_PATCH_MARKER {
        return Err(AppError::new("补丁第一行必须是 *** Begin Patch"));
    }
    if last != END_PATCH_MARKER {
        return Err(AppError::new("补丁最后一行必须是 *** End Patch"));
    }
    Ok(())
}

fn parse_one_hunk(lines: &[String], line_number: usize) -> AppResult<(PatchHunk, usize)> {
    if lines.is_empty() {
        return Err(AppError::new(format!(
            "补丁在第 {line_number} 行出现空 hunk"
        )));
    }

    let header = lines[0].trim();
    if let Some(path) = header.strip_prefix(ADD_FILE_MARKER) {
        let mut consumed = 1usize;
        let mut contents = Vec::new();
        while consumed < lines.len() {
            if let Some(stripped) = lines[consumed].strip_prefix('+') {
                contents.push(stripped.to_string());
                consumed += 1;
            } else {
                break;
            }
        }

        let contents = if contents.is_empty() {
            String::new()
        } else {
            format!("{}\n", contents.join("\n"))
        };

        return Ok((
            PatchHunk::Add {
                path: path.to_string(),
                contents,
            },
            consumed,
        ));
    }

    if let Some(path) = header.strip_prefix(DELETE_FILE_MARKER) {
        return Ok((
            PatchHunk::Delete {
                path: path.to_string(),
            },
            1,
        ));
    }

    if let Some(path) = header.strip_prefix(UPDATE_FILE_MARKER) {
        let mut consumed = 1usize;
        let mut move_path = None;
        if consumed < lines.len() {
            let move_line = lines[consumed].trim();
            if let Some(value) = move_line.strip_prefix(MOVE_TO_MARKER) {
                move_path = Some(value.to_string());
                consumed += 1;
            }
        }

        let mut chunks = Vec::new();
        while consumed < lines.len() {
            let line = &lines[consumed];
            if line.trim().is_empty() {
                consumed += 1;
                continue;
            }
            if line.starts_with("***") {
                break;
            }

            let (chunk, chunk_lines) = parse_update_file_chunk(
                &lines[consumed..],
                line_number + consumed,
                chunks.is_empty(),
            )?;
            chunks.push(chunk);
            consumed += chunk_lines;
        }

        if chunks.is_empty() {
            return Err(AppError::new(format!(
                "补丁在第 {line_number} 行更新文件 {path} 时没有有效内容"
            )));
        }

        return Ok((
            PatchHunk::Update {
                path: path.to_string(),
                move_path,
                chunks,
            },
            consumed,
        ));
    }

    Err(AppError::new(format!(
        "补丁第 {line_number} 行 hunk 头无效: {}",
        lines[0]
    )))
}

fn parse_update_file_chunk(
    lines: &[String],
    line_number: usize,
    allow_missing_context: bool,
) -> AppResult<(UpdateFileChunk, usize)> {
    if lines.is_empty() {
        return Err(AppError::new(format!(
            "补丁在第 {line_number} 行缺少更新块内容"
        )));
    }

    let mut change_context = None;
    let mut start_index = 0usize;
    if lines[0] == EMPTY_CHANGE_CONTEXT_MARKER {
        start_index = 1;
    } else if let Some(context) = lines[0].strip_prefix(CHANGE_CONTEXT_MARKER) {
        start_index = 1;
        change_context = Some(context.to_string());
    } else if !allow_missing_context {
        return Err(AppError::new(format!(
            "补丁在第 {line_number} 行预期 @@ 上下文标记"
        )));
    }

    if start_index >= lines.len() {
        return Err(AppError::new(format!(
            "补丁在第 {line_number} 行更新块为空"
        )));
    }

    let mut chunk = UpdateFileChunk {
        change_context,
        old_lines: Vec::new(),
        new_lines: Vec::new(),
        is_end_of_file: false,
    };

    let mut parsed_lines = 0usize;
    for line in &lines[start_index..] {
        if line == EOF_MARKER {
            if parsed_lines == 0 {
                return Err(AppError::new(format!(
                    "补丁在第 {line_number} 行更新块为空"
                )));
            }
            chunk.is_end_of_file = true;
            parsed_lines += 1;
            break;
        }

        match line.chars().next() {
            None => {
                chunk.old_lines.push(String::new());
                chunk.new_lines.push(String::new());
                parsed_lines += 1;
            }
            Some(' ') => {
                let content = line[1..].to_string();
                chunk.old_lines.push(content.clone());
                chunk.new_lines.push(content);
                parsed_lines += 1;
            }
            Some('+') => {
                chunk.new_lines.push(line[1..].to_string());
                parsed_lines += 1;
            }
            Some('-') => {
                chunk.old_lines.push(line[1..].to_string());
                parsed_lines += 1;
            }
            Some(_) => {
                if parsed_lines == 0 {
                    return Err(AppError::new(format!(
                        "补丁在第 {line_number} 行更新块存在非法行: {line}"
                    )));
                }
                break;
            }
        }
    }

    Ok((chunk, start_index + parsed_lines))
}

fn apply_update_hunk(file_path: &Path, chunks: &[UpdateFileChunk]) -> AppResult<String> {
    let original_contents = std::fs::read_to_string(file_path).map_err(|error| {
        AppError::new(format!(
            "读取待更新文件失败 {}: {}",
            file_path.display(),
            error
        ))
    })?;

    let mut original_lines = original_contents
        .split('\n')
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if original_lines.last().is_some_and(|line| line.is_empty()) {
        original_lines.pop();
    }

    let replacements = compute_replacements(&original_lines, file_path, chunks)?;
    let mut updated_lines = apply_replacements(&original_lines, &replacements);
    if updated_lines.last().map(|line| line.is_empty()) != Some(true) {
        updated_lines.push(String::new());
    }

    Ok(updated_lines.join("\n"))
}

fn compute_replacements(
    original_lines: &[String],
    file_path: &Path,
    chunks: &[UpdateFileChunk],
) -> AppResult<Vec<Replacement>> {
    let mut replacements = Vec::new();
    let mut line_index = 0usize;

    for chunk in chunks {
        if let Some(context) = &chunk.change_context {
            let context_slice = vec![context.clone()];
            let ctx_index = seek_sequence(original_lines, &context_slice, line_index, false)
                .ok_or_else(|| {
                    AppError::new(format!(
                        "在文件 {} 中找不到上下文 {}",
                        file_path.display(),
                        context
                    ))
                })?;
            line_index = ctx_index + 1;
        }

        if chunk.old_lines.is_empty() {
            replacements.push(Replacement {
                start_index: original_lines.len(),
                old_len: 0,
                new_lines: chunk.new_lines.clone(),
            });
            continue;
        }

        let mut pattern = chunk.old_lines.clone();
        let mut new_slice = chunk.new_lines.clone();
        let mut found = seek_sequence(original_lines, &pattern, line_index, chunk.is_end_of_file);

        if found.is_none() && pattern.last().is_some_and(|line| line.is_empty()) {
            pattern.pop();
            if new_slice.last().is_some_and(|line| line.is_empty()) {
                new_slice.pop();
            }
            found = seek_sequence(original_lines, &pattern, line_index, chunk.is_end_of_file);
        }

        let found = found.ok_or_else(|| {
            AppError::new(format!(
                "在文件 {} 中找不到预期内容:\n{}",
                file_path.display(),
                chunk.old_lines.join("\n")
            ))
        })?;

        replacements.push(Replacement {
            start_index: found,
            old_len: pattern.len(),
            new_lines: new_slice,
        });
        line_index = found + pattern.len();
    }

    replacements.sort_by_key(|replacement| replacement.start_index);
    Ok(replacements)
}

fn apply_replacements(lines: &[String], replacements: &[Replacement]) -> Vec<String> {
    let mut result = lines.to_vec();
    for replacement in replacements.iter().rev() {
        result.splice(
            replacement.start_index..replacement.start_index + replacement.old_len,
            replacement.new_lines.clone(),
        );
    }
    result
}

fn seek_sequence(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    if pattern.len() > lines.len() {
        return None;
    }

    let max_start = lines.len() - pattern.len();
    let search_start = if eof { max_start } else { start };
    if search_start > max_start {
        return None;
    }

    for index in search_start..=max_start {
        if lines_match(lines, pattern, index, |value| value.to_string()) {
            return Some(index);
        }
    }
    for index in search_start..=max_start {
        if lines_match(lines, pattern, index, |value| value.trim_end().to_string()) {
            return Some(index);
        }
    }
    for index in search_start..=max_start {
        if lines_match(lines, pattern, index, |value| value.trim().to_string()) {
            return Some(index);
        }
    }
    for index in search_start..=max_start {
        if lines_match(lines, pattern, index, |value| {
            normalize_punctuation(value.trim())
        }) {
            return Some(index);
        }
    }

    None
}

fn lines_match<F>(lines: &[String], pattern: &[String], start: usize, normalize: F) -> bool
where
    F: Fn(&str) -> String,
{
    for index in 0..pattern.len() {
        if normalize(&lines[start + index]) != normalize(&pattern[index]) {
            return false;
        }
    }
    true
}

fn normalize_punctuation(value: &str) -> String {
    value
        .chars()
        .map(|char| match char {
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
            | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}'
            | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

fn push_unique(target: &mut Vec<String>, seen: &mut BTreeSet<String>, value: String) {
    if seen.insert(value.clone()) {
        target.push(value);
    }
}

fn format_patch_summary(summary: &PatchSummary) -> String {
    let mut lines = vec!["Success. Updated the following files:".to_string()];
    for path in &summary.added {
        lines.push(format!("A {path}"));
    }
    for path in &summary.modified {
        lines.push(format!("M {path}"));
    }
    for path in &summary.deleted {
        lines.push(format!("D {path}"));
    }
    lines.join("\n")
}

fn to_display_path(path: &Path, root: &Path) -> String {
    let normalized_path = canonicalize_if_exists_or_normalize(path);
    let normalized_root = canonicalize_if_exists_or_normalize(root);

    let relative = normalized_path
        .strip_prefix(&normalized_root)
        .or_else(|_| path.strip_prefix(root))
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .ok();

    match relative {
        Some(value) if !value.is_empty() => value,
        Some(_) => path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        None => path.display().to_string(),
    }
}

fn canonicalize_if_exists_or_normalize(path: &Path) -> PathBuf {
    if path.exists() {
        std::fs::canonicalize(path).unwrap_or_else(|_| normalize_path(path))
    } else {
        normalize_path(path)
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let last = normalized.components().next_back();
                if last.is_some_and(|value| matches!(value, Component::Normal(_))) {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }
    normalized
}

fn is_within_root(path: &Path, root: &Path) -> bool {
    let normalized_path = normalize_path(path);
    let normalized_root = normalize_path(root);
    normalized_path.starts_with(&normalized_root)
}

fn is_likely_binary(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if bytes.contains(&0) {
        return true;
    }

    let sample = &bytes[..bytes.len().min(1024)];
    let non_text = sample
        .iter()
        .filter(|byte| !matches!(byte, b'\n' | b'\r' | b'\t' | 0x20..=0x7E))
        .count();
    non_text * 10 > sample.len() * 3
}

fn preview_text(value: &str) -> String {
    let compact = value.replace('\n', "\\n");
    if compact.chars().count() <= 120 {
        compact
    } else {
        format!("{}...", compact.chars().take(120).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hash_annotated_file() {
        let dir = std::env::temp_dir().join(format!("codeforge-file-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should exist");
        let file = dir.join("sample.txt");
        std::fs::write(&file, "hello\nworld").expect("sample file should exist");

        let content = read_file(&file).expect("file should be readable");
        assert!(content.contains("1#"));
        assert!(content.contains("|hello"));
    }

    #[test]
    fn writes_and_patches_file() {
        let dir =
            std::env::temp_dir().join(format!("codeforge-file-write-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should exist");
        let file = dir.join("sample.txt");
        write_file(&file, "alpha beta").expect("file should be written");
        let updated = apply_patch_text(&file, "beta", "gamma").expect("patch should apply");
        assert!(updated.contains("gamma"));
    }

    #[test]
    fn applies_structured_patch_with_update_hunk() {
        let dir =
            std::env::temp_dir().join(format!("codeforge-file-patch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should exist");
        let file = dir.join("demo.txt");
        std::fs::write(&file, "alpha\nbeta\ngamma\n").expect("seed file should exist");

        let patch =
            "*** Begin Patch\n*** Update File: demo.txt\n-beta\n+beta-updated\n*** End Patch";
        let summary = apply_structured_patch(&dir, patch).expect("patch should apply");

        let updated = std::fs::read_to_string(&file).expect("patched file should be readable");
        assert!(summary.contains("M demo.txt"));
        assert!(updated.contains("beta-updated"));
    }

    #[test]
    fn blocks_path_escape_when_root_is_provided() {
        let dir =
            std::env::temp_dir().join(format!("codeforge-file-root-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir should exist");

        let error = resolve_path(Some(&dir), "../escape.txt").expect_err("path should be denied");
        assert!(error.message.contains("路径超出工作区"));
    }
}
