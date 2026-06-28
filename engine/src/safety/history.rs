//! Local file history stored under `.gwenland/history/`.
//!
//! This is deliberately independent of Git: every save/manual/AI snapshot is a
//! plain text file plus a small per-file JSONL index. Missing/malformed index
//! lines are skipped so the history panel can fail open.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const DEFAULT_HISTORY_MAX_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("path would escape workspace root")]
    OutsideWorkspace,
    #[error("history entry not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub size: u64,
    pub source: String,
}

#[derive(Debug)]
struct IndexedEntry {
    index_path: PathBuf,
    content_path: PathBuf,
    entry: HistoryEntry,
}

fn now_history_timestamp() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}-{:02}-{:02}.{:09}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.nanosecond()
    )
}

fn assert_inside(file_path: &Path, workspace_root: &Path) -> Result<(), HistoryError> {
    if crate::agentic::policy::is_within_workspace(file_path, workspace_root) {
        Ok(())
    } else {
        Err(HistoryError::OutsideWorkspace)
    }
}

fn relative_slash(workspace_root: &Path, file_path: &Path) -> Result<String, HistoryError> {
    assert_inside(file_path, workspace_root)?;
    let rel = file_path
        .strip_prefix(workspace_root)
        .map_err(|_| HistoryError::OutsideWorkspace)?;
    Ok(rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn sanitize_segment(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches(['.', '-', '_']).to_string();
    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed
    }
}

fn sanitized_relative_path(workspace_root: &Path, file_path: &Path) -> String {
    relative_slash(workspace_root, file_path)
        .unwrap_or_else(|_| {
            file_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".to_string())
        })
        .split('/')
        .map(sanitize_segment)
        .collect::<Vec<_>>()
        .join("__")
}

pub fn history_dir(workspace_root: &Path, file_path: &Path) -> PathBuf {
    crate::workspace::gwenland_dir(workspace_root)
        .join("history")
        .join(sanitized_relative_path(workspace_root, file_path))
}

fn index_path(workspace_root: &Path, file_path: &Path) -> PathBuf {
    history_dir(workspace_root, file_path).join("index.jsonl")
}

fn entry_path(workspace_root: &Path, file_path: &Path, timestamp: &str) -> PathBuf {
    history_dir(workspace_root, file_path).join(format!("{timestamp}.txt"))
}

fn append_index(path: &Path, entry: &HistoryEntry) -> Result<(), HistoryError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_string(entry)?;
    line.push('\n');
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

fn read_index(path: &Path) -> Vec<HistoryEntry> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => return Vec::new(),
    };
    raw.lines()
        .filter_map(|line| serde_json::from_str::<HistoryEntry>(line).ok())
        .collect()
}

fn read_settings_json(workspace_root: &Path) -> Option<serde_json::Value> {
    let raw = fs::read_to_string(crate::workspace::settings_path(workspace_root)).ok()?;
    serde_json::from_str(&raw).ok()
}

fn configured_max_bytes(workspace_root: &Path) -> u64 {
    let Some(value) = read_settings_json(workspace_root) else {
        return DEFAULT_HISTORY_MAX_BYTES;
    };
    value
        .pointer("/history/maxBytes")
        .and_then(|v| v.as_u64())
        .or_else(|| value.get("history_max_bytes").and_then(|v| v.as_u64()))
        .unwrap_or(DEFAULT_HISTORY_MAX_BYTES)
}

fn configured_excludes(workspace_root: &Path) -> Vec<String> {
    let mut patterns = vec![
        ".env".to_string(),
        ".env.*".to_string(),
        ".gwenland/**".to_string(),
    ];
    let Some(value) = read_settings_json(workspace_root) else {
        return patterns;
    };
    if let Some(extra) = value
        .pointer("/history/excludePatterns")
        .and_then(|v| v.as_array())
        .or_else(|| {
            value
                .get("history_exclude_patterns")
                .and_then(|v| v.as_array())
        })
    {
        patterns.extend(extra.iter().filter_map(|v| v.as_str().map(str::to_string)));
    }
    patterns
}

fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path == prefix || path.starts_with(&(prefix.to_string() + "/"));
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return path.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return path.ends_with(suffix);
    }
    path == pattern
}

fn is_excluded(workspace_root: &Path, file_path: &Path) -> Result<bool, HistoryError> {
    let rel = relative_slash(workspace_root, file_path)?;
    Ok(configured_excludes(workspace_root)
        .iter()
        .any(|pattern| glob_match(pattern, &rel)))
}

pub fn save_history_entry(
    workspace_root: &Path,
    file_path: &Path,
    content: &str,
    source: &str,
) -> Result<Option<HistoryEntry>, HistoryError> {
    assert_inside(file_path, workspace_root)?;
    if is_excluded(workspace_root, file_path)? {
        return Ok(None);
    }

    let dir = history_dir(workspace_root, file_path);
    fs::create_dir_all(&dir)?;
    let timestamp = now_history_timestamp();
    let content_path = dir.join(format!("{timestamp}.txt"));
    fs::write(&content_path, content)?;
    let entry = HistoryEntry {
        timestamp,
        size: content.len() as u64,
        source: source.to_string(),
    };
    append_index(&dir.join("index.jsonl"), &entry)?;
    enforce_size_limit(workspace_root, configured_max_bytes(workspace_root))?;
    Ok(Some(entry))
}

pub fn list_history(
    workspace_root: &Path,
    file_path: &Path,
) -> Result<Vec<HistoryEntry>, HistoryError> {
    assert_inside(file_path, workspace_root)?;
    let mut entries: Vec<_> = read_index(&index_path(workspace_root, file_path))
        .into_iter()
        .filter(|entry| entry_path(workspace_root, file_path, &entry.timestamp).exists())
        .collect();
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(entries)
}

pub fn read_history_entry(
    workspace_root: &Path,
    file_path: &Path,
    timestamp: &str,
) -> Result<String, HistoryError> {
    assert_inside(file_path, workspace_root)?;
    let path = entry_path(workspace_root, file_path, timestamp);
    if !path.exists() {
        return Err(HistoryError::NotFound(timestamp.to_string()));
    }
    Ok(fs::read_to_string(path)?)
}

pub fn clear_history(workspace_root: &Path, file_path: &Path) -> Result<(), HistoryError> {
    assert_inside(file_path, workspace_root)?;
    let dir = history_dir(workspace_root, file_path);
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}

fn collect_index_paths(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), HistoryError> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_index_paths(&path, out)?;
        } else if path.file_name().and_then(|n| n.to_str()) == Some("index.jsonl") {
            out.push(path);
        }
    }
    Ok(())
}

fn collect_entries(workspace_root: &Path) -> Result<Vec<IndexedEntry>, HistoryError> {
    let root = crate::workspace::gwenland_dir(workspace_root).join("history");
    let mut indexes = Vec::new();
    collect_index_paths(&root, &mut indexes)?;
    let mut entries = Vec::new();
    for index in indexes {
        let Some(dir) = index.parent() else {
            continue;
        };
        for entry in read_index(&index) {
            let content_path = dir.join(format!("{}.txt", entry.timestamp));
            if content_path.exists() {
                entries.push(IndexedEntry {
                    index_path: index.clone(),
                    content_path,
                    entry,
                });
            }
        }
    }
    Ok(entries)
}

fn rewrite_indexes(entries: &[IndexedEntry]) -> Result<(), HistoryError> {
    let mut grouped: std::collections::BTreeMap<PathBuf, Vec<&HistoryEntry>> =
        std::collections::BTreeMap::new();
    for entry in entries {
        if entry.content_path.exists() {
            grouped
                .entry(entry.index_path.clone())
                .or_default()
                .push(&entry.entry);
        }
    }
    for (index, rows) in grouped {
        let mut content = String::new();
        for row in rows {
            content.push_str(&serde_json::to_string(row)?);
            content.push('\n');
        }
        fs::write(index, content)?;
    }
    Ok(())
}

fn enforce_size_limit(workspace_root: &Path, max_bytes: u64) -> Result<(), HistoryError> {
    let mut entries = collect_entries(workspace_root)?;
    let mut total: u64 = entries.iter().map(|entry| entry.entry.size).sum();
    if total <= max_bytes {
        return Ok(());
    }
    entries.sort_by(|a, b| a.entry.timestamp.cmp(&b.entry.timestamp));
    for entry in &entries {
        if total <= max_bytes {
            break;
        }
        let _ = fs::remove_file(&entry.content_path);
        total = total.saturating_sub(entry.entry.size);
    }
    rewrite_indexes(&entries)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_dir_sanitizes_relative_path() {
        let ws = tempfile::tempdir().unwrap();
        let file = ws.path().join("src").join("main.rs");
        let dir = history_dir(ws.path(), &file);
        assert!(
            dir.to_string_lossy()
                .replace('\\', "/")
                .ends_with(".gwenland/history/src__main.rs")
        );
    }

    #[test]
    fn save_list_read_and_clear_round_trip() {
        let ws = tempfile::tempdir().unwrap();
        let file = ws.path().join("src/main.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "current").unwrap();
        let entry = save_history_entry(ws.path(), &file, "one", "manual")
            .unwrap()
            .unwrap();
        assert_eq!(entry.source, "manual");
        let list = list_history(ws.path(), &file).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            read_history_entry(ws.path(), &file, &list[0].timestamp).unwrap(),
            "one"
        );
        clear_history(ws.path(), &file).unwrap();
        assert!(list_history(ws.path(), &file).unwrap().is_empty());
    }

    #[test]
    fn env_and_gwenland_files_are_excluded() {
        let ws = tempfile::tempdir().unwrap();
        let env = ws.path().join(".env");
        fs::write(&env, "SECRET=1").unwrap();
        assert!(
            save_history_entry(ws.path(), &env, "SECRET=1", "save")
                .unwrap()
                .is_none()
        );
        let internal = ws.path().join(".gwenland/settings.json");
        fs::create_dir_all(internal.parent().unwrap()).unwrap();
        fs::write(&internal, "{}").unwrap();
        assert!(
            save_history_entry(ws.path(), &internal, "{}", "save")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn configured_excludes_are_honored() {
        let ws = tempfile::tempdir().unwrap();
        fs::create_dir_all(ws.path().join(".gwenland")).unwrap();
        fs::write(
            ws.path().join(".gwenland/settings.json"),
            r#"{"history":{"excludePatterns":["secrets/**"]}}"#,
        )
        .unwrap();
        let secret = ws.path().join("secrets/token.txt");
        fs::create_dir_all(secret.parent().unwrap()).unwrap();
        fs::write(&secret, "token").unwrap();
        assert!(
            save_history_entry(ws.path(), &secret, "token", "save")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn size_limit_prunes_oldest() {
        let ws = tempfile::tempdir().unwrap();
        fs::create_dir_all(ws.path().join(".gwenland")).unwrap();
        fs::write(
            ws.path().join(".gwenland/settings.json"),
            r#"{"history":{"maxBytes":8}}"#,
        )
        .unwrap();
        let file = ws.path().join("a.txt");
        fs::write(&file, "").unwrap();
        save_history_entry(ws.path(), &file, "12345", "save").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        save_history_entry(ws.path(), &file, "67890", "save").unwrap();
        let list = list_history(ws.path(), &file).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            read_history_entry(ws.path(), &file, &list[0].timestamp).unwrap(),
            "67890"
        );
    }

    #[test]
    fn outside_workspace_is_rejected() {
        let ws = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        assert!(matches!(
            save_history_entry(ws.path(), outside.path(), "x", "save"),
            Err(HistoryError::OutsideWorkspace)
        ));
    }
}
