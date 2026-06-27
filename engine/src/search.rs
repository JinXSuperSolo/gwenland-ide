//! Workspace text search (M19 Wave 8).
//!
//! Pure std implementation: recursive directory walk, policy exclusions, and
//! streamed line matches through a callback. Tauri owns event emission and
//! cancellation wiring; this module only observes an `AtomicBool`.

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, ErrorKind};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::search_policy::should_exclude_fast;

pub const DEFAULT_MAX_RESULTS: usize = 2_000;
const MAX_LINE_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub relative_path: String,
    /// 1-based line number.
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchSummary {
    pub result_count: usize,
    pub scanned_files: usize,
    pub cancelled: bool,
    pub truncated: bool,
}

impl SearchSummary {
    fn new() -> Self {
        Self {
            result_count: 0,
            scanned_files: 0,
            cancelled: false,
            truncated: false,
        }
    }
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn trim_line(line: &str) -> String {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.len() <= MAX_LINE_BYTES {
        trimmed.to_string()
    } else {
        let prefix: String = trimmed.chars().take(MAX_LINE_BYTES).collect();
        format!("{prefix}...")
    }
}

fn sorted_entries(dir: &Path) -> io::Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by(|a, b| {
        let an = a.file_name().to_string_lossy().to_lowercase();
        let bn = b.file_name().to_string_lossy().to_lowercase();
        an.cmp(&bn)
    });
    Ok(entries)
}

fn scan_file<F>(
    root: &Path,
    path: &Path,
    query_lower: &str,
    cancel: &AtomicBool,
    summary: &mut SearchSummary,
    on_result: &mut F,
) -> io::Result<()>
where
    F: FnMut(SearchResult),
{
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return Ok(()),
    };
    summary.scanned_files += 1;

    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut line_number = 0usize;
    loop {
        if cancel.load(Ordering::Relaxed) {
            summary.cancelled = true;
            return Ok(());
        }
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return Ok(()),
            Ok(_) => {
                line_number += 1;
                if line.to_lowercase().contains(query_lower) {
                    let result = SearchResult {
                        path: path_string(path),
                        relative_path: rel_path(root, path),
                        line_number,
                        line: trim_line(&line),
                    };
                    on_result(result);
                    summary.result_count += 1;
                    if summary.result_count >= DEFAULT_MAX_RESULTS {
                        summary.truncated = true;
                        return Ok(());
                    }
                }
            }
            Err(err) if err.kind() == ErrorKind::InvalidData => return Ok(()),
            Err(_) => return Ok(()),
        }
    }
}

pub fn search_workspace<F>(
    root: &Path,
    query: &str,
    cancel: Arc<AtomicBool>,
    mut on_result: F,
) -> io::Result<SearchSummary>
where
    F: FnMut(SearchResult),
{
    let mut summary = SearchSummary::new();
    let query = query.trim();
    if query.is_empty() {
        return Ok(summary);
    }
    let query_lower = query.to_lowercase();

    if !root.is_dir() {
        return Err(io::Error::new(
            ErrorKind::NotFound,
            format!("workspace root not found: {}", root.display()),
        ));
    }

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            summary.cancelled = true;
            break;
        }

        let entries = match sorted_entries(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.into_iter().rev() {
            if cancel.load(Ordering::Relaxed) {
                summary.cancelled = true;
                return Ok(summary);
            }

            let path = entry.path();
            let rel = rel_path(root, &path);
            if !rel.is_empty() && should_exclude_fast(&rel) {
                continue;
            }

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() {
                scan_file(
                    root,
                    &path,
                    &query_lower,
                    &cancel,
                    &mut summary,
                    &mut on_result,
                )?;
                if summary.cancelled || summary.truncated {
                    return Ok(summary);
                }
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tempfile::tempdir;

    #[test]
    fn streams_line_matches_with_relative_paths() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("src").join("main.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "fn main() {}\nlet needle = true;\n").unwrap();

        let mut results = Vec::new();
        let summary = search_workspace(
            dir.path(),
            "NEEDLE",
            Arc::new(AtomicBool::new(false)),
            |result| results.push(result),
        )
        .unwrap();

        assert_eq!(summary.result_count, 1);
        assert_eq!(summary.scanned_files, 1);
        assert_eq!(results[0].relative_path, "src/main.rs");
        assert_eq!(results[0].line_number, 2);
        assert_eq!(results[0].line, "let needle = true;");
    }

    #[test]
    fn excludes_generated_and_vcs_dirs() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("node_modules/pkg/a.js"), "needle").unwrap();
        fs::write(dir.path().join(".git/config"), "needle").unwrap();
        fs::write(dir.path().join("src/lib.rs"), "needle").unwrap();

        let mut results = Vec::new();
        let summary = search_workspace(
            dir.path(),
            "needle",
            Arc::new(AtomicBool::new(false)),
            |result| results.push(result),
        )
        .unwrap();

        assert_eq!(summary.result_count, 1);
        assert_eq!(results[0].relative_path, "src/lib.rs");
    }

    #[test]
    fn observes_pre_cancelled_flag() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "needle").unwrap();
        let cancel = Arc::new(AtomicBool::new(true));

        let summary = search_workspace(dir.path(), "needle", cancel, |_| {}).unwrap();

        assert!(summary.cancelled);
        assert_eq!(summary.result_count, 0);
    }
}
