//! M13 local memory store (GWEN-335).
//!
//! Pure path helpers, sanitization, keyword-grep, ranking, memory block rendering,
//! note writing, and mini-call JSON parsing. Zero Tauri / UI / network imports.
//!
//! Memory lives at:
//!   `<workspace-root>/.gwenland/agent/memory/<project-name>/<conversation-name>/<topic>.md`

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum MemoryError {
    Io(std::io::Error),
    InvalidNote(String),
    OutsideWorkspace,
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::Io(e) => write!(f, "memory I/O: {e}"),
            MemoryError::InvalidNote(msg) => write!(f, "invalid memory note: {msg}"),
            MemoryError::OutsideWorkspace => write!(f, "memory path escapes workspace"),
        }
    }
}

impl From<std::io::Error> for MemoryError {
    fn from(e: std::io::Error) -> Self {
        MemoryError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// One memory file hit from a keyword grep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    /// Workspace-relative path of the matched file.
    pub filename: String,
    /// Bounded set of matching lines from the file.
    pub matched_lines: Vec<String>,
    /// Relevance score (higher = better).
    pub score: usize,
}

/// A short markdown note returned by the write-back mini-call.
#[derive(Debug, Clone)]
pub struct MemoryNote {
    /// AI-generated kebab-case filename (ends in `.md`).
    pub filename: String,
    /// Markdown content, 10-15 lines.
    pub content: String,
}

/// Identifies where to write a memory note.
#[derive(Debug, Clone)]
pub struct MemoryWriteTarget {
    pub project_name: String,
    pub conversation_name: String,
    pub filename: String,
}

/// Character budget for rendered memory context blocks.
#[derive(Debug, Clone, Copy)]
pub struct MemoryBudget {
    /// Soft cap on the total rendered `<memory>` block (chars, ~4 chars/token).
    pub max_chars: usize,
    /// Maximum matched lines extracted per result file.
    pub max_lines_per_file: usize,
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self {
            max_chars: 3000, // ≈750 tokens
            max_lines_per_file: 6,
        }
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Return the project-scoped memory directory.
pub fn memory_project_dir(workspace_root: &Path, project_name: &str) -> PathBuf {
    workspace_root
        .join(".gwenland")
        .join("agent")
        .join("memory")
        .join(project_name)
}

/// Return the conversation-scoped memory directory.
pub fn memory_conversation_dir(
    workspace_root: &Path,
    project_name: &str,
    conversation_name: &str,
) -> PathBuf {
    memory_project_dir(workspace_root, project_name).join(conversation_name)
}

/// Derive the project name segment from the workspace root folder name.
pub fn project_name_from_root(workspace_root: &Path) -> String {
    let raw = workspace_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    sanitize_segment(&raw, "project")
}

// ---------------------------------------------------------------------------
// Sanitization
// ---------------------------------------------------------------------------

/// Sanitize a path segment into safe kebab-case.
///
/// Rules:
/// - Lowercase.
/// - Whitespace and common separators → `-`.
/// - Keep only `a-z`, `0-9`, `-`, `_`, `.`.
/// - Collapse repeated `-`.
/// - Trim leading/trailing `-` and `.`.
/// - Reject path separators and traversal (`/`, `\`, `..`).
/// - If the result is empty, return `fallback`.
pub fn sanitize_segment(input: &str, fallback: &str) -> String {
    // Reject obvious traversal attempts early.
    let input = input.replace("..", "").replace(['/', '\\'], "-");
    let lower = input.to_lowercase();

    let mut out = String::with_capacity(lower.len());
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            out.push(ch);
        } else if ch == '-'
            || ch.is_ascii_whitespace()
            || matches!(
                ch,
                '+' | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | ','
                    | ';'
                    | ':'
                    | '!'
                    | '?'
                    | '&'
                    | '#'
                    | '%'
                    | '@'
                    | '='
                    | '~'
                    | '^'
                    | '*'
                    | '|'
            )
        {
            // Treat all separators as a dash (we'll collapse them).
            out.push('-');
        }
        // Other chars (non-ASCII, control, etc.) are dropped.
    }

    // Collapse repeated dashes.
    let mut result = String::with_capacity(out.len());
    let mut prev_dash = false;
    for ch in out.chars() {
        if ch == '-' {
            if !prev_dash {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(ch);
            prev_dash = false;
        }
    }

    // Trim leading/trailing `-` and `.`.
    let result = result.trim_matches(|c| c == '-' || c == '.').to_string();

    if result.is_empty() {
        fallback.to_string()
    } else {
        result
    }
}

/// Ensure a note filename ends in `.md` and is safe.
pub fn sanitize_note_filename(raw: &str) -> String {
    // Strip any directory component.
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let stem = base.trim_end_matches(".md");
    let clean = sanitize_segment(stem, "memory-note");
    format!("{clean}.md")
}

// ---------------------------------------------------------------------------
// Memory grep
// ---------------------------------------------------------------------------

/// Maximum files walked inside the memory directory.
const MAX_MEMORY_WALK: usize = 500;
/// Maximum total matched lines across all results.
const MAX_TOTAL_MATCHED: usize = 200;
/// Maximum bytes per file we bother reading.
const MAX_FILE_BYTES: u64 = 64 * 1024;

/// Search `.gwenland/agent/memory/<project-name>/**/*.md` for `keywords`.
///
/// Returns results sorted by descending score. Returns `Ok([])` when no
/// keywords are provided or no notes exist.
pub fn search_memory(
    workspace_root: &Path,
    project_name: &str,
    keywords: &[String],
    budget: MemoryBudget,
) -> Result<Vec<MemorySearchResult>, MemoryError> {
    if keywords.is_empty() {
        return Ok(Vec::new());
    }

    let project_dir = memory_project_dir(workspace_root, project_name);
    if !project_dir.is_dir() {
        return Ok(Vec::new());
    }

    let normalized: Vec<String> = keywords.iter().map(|k| k.to_ascii_lowercase()).collect();

    let mut results: Vec<MemorySearchResult> = Vec::new();
    let mut scanned = 0usize;
    let mut total_matched_lines = 0usize;

    walk_memory_dir(
        &project_dir,
        &project_dir,
        workspace_root,
        &mut scanned,
        &mut |rel_path: &str, abs_path: &Path| {
            if total_matched_lines >= MAX_TOTAL_MATCHED {
                return false;
            }
            let content = match fs::read_to_string(abs_path) {
                Ok(c) => c,
                Err(_) => return true, // skip binary/unreadable
            };

            let mut matched_lines: Vec<String> = Vec::new();
            let mut score = 0usize;

            // Score from filename.
            let filename_lower = rel_path.to_ascii_lowercase();
            for kw in &normalized {
                if filename_lower.contains(kw.as_str()) {
                    score += 10;
                }
            }

            // Score from content lines.
            for (i, line) in content.lines().enumerate() {
                let line_lower = line.to_ascii_lowercase();
                let is_heading = line.starts_with('#');

                let mut line_kw_hits = 0usize;
                for kw in &normalized {
                    if line_lower.contains(kw.as_str()) {
                        line_kw_hits += 1;
                    }
                }

                if line_kw_hits > 0 {
                    let weight = if is_heading { 5 } else { 2 };
                    score += weight * line_kw_hits;

                    if matched_lines.len() < budget.max_lines_per_file {
                        let trimmed = line.trim();
                        let (snippet, _) = truncate_str(trimmed, 200);
                        matched_lines.push(format!("{}: {snippet}", i + 1));
                    }
                }
            }

            // Bonus for covering multiple keywords.
            let covered = normalized
                .iter()
                .filter(|kw| {
                    filename_lower.contains(kw.as_str())
                        || content.to_ascii_lowercase().contains(kw.as_str())
                })
                .count();
            if covered > 1 {
                score += (covered - 1) * 3;
            }

            if score > 0 && !matched_lines.is_empty() {
                total_matched_lines += matched_lines.len();
                results.push(MemorySearchResult {
                    filename: rel_path.to_string(),
                    matched_lines,
                    score,
                });
            }
            true
        },
    );

    // Sort by score desc, then filename asc for deterministic tie-breaking.
    results.sort_by(|a, b| b.score.cmp(&a.score).then(a.filename.cmp(&b.filename)));

    // Apply character budget.
    apply_char_budget(&mut results, budget.max_chars);

    Ok(results)
}

/// Walk only `.md` files inside the memory directory. Stops if more than
/// `MAX_MEMORY_WALK` files have been scanned or `visit` returns false.
/// `visit(workspace_relative_path, absolute_path) -> should_continue`
#[allow(clippy::only_used_in_recursion)]
fn walk_memory_dir(
    mem_root: &Path,
    dir: &Path,
    workspace_root: &Path,
    scanned: &mut usize,
    visit: &mut impl FnMut(&str, &Path) -> bool,
) -> bool {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return true,
    };
    for entry in entries.flatten() {
        let ft = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let path = entry.path();
        if ft.is_dir() {
            if !walk_memory_dir(mem_root, &path, workspace_root, scanned, visit) {
                return false;
            }
        } else if ft.is_file() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !name.ends_with(".md") {
                continue;
            }
            // Skip oversized files.
            if let Ok(meta) = entry.metadata()
                && meta.len() > MAX_FILE_BYTES
            {
                continue;
            }
            *scanned += 1;
            if *scanned > MAX_MEMORY_WALK {
                return false;
            }
            // Build workspace-relative path.
            let rel = path
                .strip_prefix(workspace_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if !visit(&rel, &path) {
                return false;
            }
        }
    }
    true
}

/// Prune results from the tail until the rendered block fits within `max_chars`.
fn apply_char_budget(results: &mut Vec<MemorySearchResult>, max_chars: usize) {
    while !results.is_empty() {
        let rendered = render_memory_block_inner(results);
        if rendered.len() <= max_chars {
            break;
        }
        results.pop();
    }
}

// ---------------------------------------------------------------------------
// Memory block rendering
// ---------------------------------------------------------------------------

/// Render the `<memory>...</memory>` block for injection into provider context.
/// Returns `None` when there are no results.
pub fn render_memory_block(results: &[MemorySearchResult], budget: MemoryBudget) -> Option<String> {
    if results.is_empty() {
        return None;
    }
    // Clone + budget-cap before rendering.
    let mut capped = results.to_vec();
    apply_char_budget(&mut capped, budget.max_chars);
    if capped.is_empty() {
        return None;
    }
    Some(render_memory_block_inner(&capped))
}

fn render_memory_block_inner(results: &[MemorySearchResult]) -> String {
    let mut out = String::from("<memory>\n");
    for result in results {
        // Use the basename as the display name.
        let name = result
            .filename
            .rsplit('/')
            .next()
            .unwrap_or(&result.filename);
        out.push('[');
        out.push_str(name);
        out.push_str("]\n");
        for line in &result.matched_lines {
            out.push_str("- ");
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str("</memory>");
    out
}

// ---------------------------------------------------------------------------
// Note writing
// ---------------------------------------------------------------------------

/// Write (or append) a memory note to the conversation directory.
///
/// - Creates missing directories.
/// - Appends with `\n\n---\n\n` separator when the file already exists.
/// - Trims trailing whitespace and caps content at `NOTE_LINE_CAP` lines.
/// - Returns the written path (for tests/debug; callers may ignore it).
pub fn write_memory_note(
    workspace_root: &Path,
    target: &MemoryWriteTarget,
    note: &MemoryNote,
) -> Result<PathBuf, MemoryError> {
    let dir = memory_conversation_dir(
        workspace_root,
        &target.project_name,
        &target.conversation_name,
    );

    // Verify the directory path stays inside the workspace before creating it.
    let workspace_canon = workspace_root
        .canonicalize()
        .map_err(|_| MemoryError::OutsideWorkspace)?;

    // For the directory, we resolve parent by parent (it may not exist yet).
    let abs_dir = workspace_root.join(dir.strip_prefix(workspace_root).unwrap_or(&dir));
    // Safety: ensure the computed path is under workspace root.
    // We compare the normalized string since the dir might not exist yet.
    let dir_str = abs_dir.to_string_lossy();
    let ws_str = workspace_canon.to_string_lossy();
    if !dir_str.starts_with(ws_str.as_ref()) && !abs_dir.starts_with(workspace_root) {
        return Err(MemoryError::OutsideWorkspace);
    }

    fs::create_dir_all(&dir)?;

    let safe_filename = sanitize_note_filename(&target.filename);
    let note_path = dir.join(&safe_filename);

    // Cap content lines.
    let content = cap_note_content(&note.content);
    if content.trim().is_empty() {
        return Err(MemoryError::InvalidNote("empty content".into()));
    }

    if note_path.is_file() {
        // Append with separator.
        let existing = fs::read_to_string(&note_path)?;
        let separator = if existing.trim_end().ends_with("---") {
            "\n"
        } else {
            "\n\n---\n\n"
        };
        let mut new_content = existing;
        new_content.push_str(separator);
        new_content.push_str(&content);
        fs::write(&note_path, new_content)?;
    } else {
        fs::write(&note_path, content)?;
    }

    Ok(note_path)
}

const NOTE_LINE_CAP: usize = 20;

fn cap_note_content(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let capped = if lines.len() > NOTE_LINE_CAP {
        &lines[..NOTE_LINE_CAP]
    } else {
        &lines
    };
    capped
        .iter()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string()
}

// ---------------------------------------------------------------------------
// Mini-call JSON parsing helpers
// ---------------------------------------------------------------------------

/// Parse a JSON array of keyword strings from raw mini-call output.
///
/// Tolerates:
/// - Surrounding whitespace.
/// - Markdown fenced blocks (` ```json ... ``` `).
/// - Non-string entries (silently dropped).
///
/// Returns an empty vec on any parse failure.
pub fn parse_keyword_array(raw: &str) -> Vec<String> {
    let stripped = strip_json_fences(raw);
    let v: serde_json::Value = match serde_json::from_str(stripped.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let arr = match v.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse `{ "filename": "...", "content": "..." }` from raw mini-call output.
///
/// Returns `Err` when content is empty or the filename is invalid.
pub fn parse_memory_note(raw: &str) -> Result<MemoryNote, MemoryError> {
    let stripped = strip_json_fences(raw);
    let v: serde_json::Value = serde_json::from_str(stripped.trim())
        .map_err(|e| MemoryError::InvalidNote(format!("JSON parse: {e}")))?;

    let filename = v
        .get("filename")
        .and_then(|f| f.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let content = v
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    if filename.is_empty() {
        return Err(MemoryError::InvalidNote("missing filename".into()));
    }
    if content.trim().is_empty() {
        return Err(MemoryError::InvalidNote("empty content".into()));
    }

    Ok(MemoryNote { filename, content })
}

/// Strip surrounding Markdown code fences (` ```json ... ``` ` etc.).
fn strip_json_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    // Remove opening fence line.
    let after_open = if let Some(rest) = trimmed.strip_prefix("```") {
        // Skip optional language tag on the same line.
        rest.trim_start_matches(|c: char| c.is_alphabetic())
            .trim_start_matches('\n')
    } else {
        return trimmed;
    };
    // Remove closing fence.
    if let Some(inner) = after_open.strip_suffix("```") {
        inner.trim()
    } else {
        after_open.trim()
    }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn truncate_str(s: &str, max_chars: usize) -> (&str, bool) {
    if s.len() <= max_chars {
        return (s, false);
    }
    let mut end = max_chars;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    (&s[..end], true)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // --- sanitize_segment ---

    #[test]
    fn sanitize_spaces_and_punctuation() {
        assert_eq!(sanitize_segment("Hello World!", "fb"), "hello-world");
        assert_eq!(sanitize_segment("fix/null-parser", "fb"), "fix-null-parser");
        assert_eq!(sanitize_segment("  --  ", "fb"), "fb"); // empty after trim
        assert_eq!(sanitize_segment("foo__bar", "fb"), "foo__bar");
    }

    #[test]
    fn sanitize_rejects_traversal() {
        // `..` should be stripped, not traversed.
        let s = sanitize_segment("../../etc/passwd", "fb");
        assert!(!s.contains(".."), "traversal leaked: {s}");
        assert!(!s.contains('/'), "slash leaked: {s}");
    }

    #[test]
    fn sanitize_windows_separators() {
        let s = sanitize_segment("src\\components\\Layout", "fb");
        assert!(!s.contains('\\'), "backslash leaked: {s}");
    }

    #[test]
    fn sanitize_empty_uses_fallback() {
        assert_eq!(sanitize_segment("", "fallback"), "fallback");
        assert_eq!(sanitize_segment("!!!", "fallback"), "fallback");
    }

    #[test]
    fn sanitize_note_filename_appends_md() {
        assert_eq!(
            sanitize_note_filename("fix-null-parser"),
            "fix-null-parser.md"
        );
        assert_eq!(
            sanitize_note_filename("fix-null-parser.md"),
            "fix-null-parser.md"
        );
        assert_eq!(sanitize_note_filename("../escape"), "escape.md");
    }

    // --- path helpers ---

    #[test]
    fn memory_dirs_are_inside_workspace() {
        let root = Path::new("/workspace");
        let proj_dir = memory_project_dir(root, "my-project");
        assert!(proj_dir.starts_with(root));
        let conv_dir = memory_conversation_dir(root, "my-project", "conv-abc");
        assert!(conv_dir.starts_with(root));
    }

    #[test]
    fn project_name_from_root_sanitizes() {
        let root = Path::new("/home/user/My Project!");
        let name = project_name_from_root(root);
        assert_eq!(name, "my-project");
    }

    // --- search_memory ---

    #[test]
    fn search_returns_empty_for_no_keywords() {
        let dir = tempdir().unwrap();
        let results = search_memory(dir.path(), "proj", &[], MemoryBudget::default()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_returns_empty_when_no_memory_dir() {
        let dir = tempdir().unwrap();
        let results =
            search_memory(dir.path(), "proj", &["foo".into()], MemoryBudget::default()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_finds_keyword_in_content() {
        let dir = tempdir().unwrap();
        let mem_dir = dir.path().join(".gwenland/agent/memory/proj/conv1");
        fs::create_dir_all(&mem_dir).unwrap();
        fs::write(
            mem_dir.join("fix-null-parser.md"),
            "# Fix Null Parser\n- Root cause: unwrap() at line 42\n- Use if let Some\n",
        )
        .unwrap();

        let results = search_memory(
            dir.path(),
            "proj",
            &["unwrap".into(), "parser".into()],
            MemoryBudget::default(),
        )
        .unwrap();
        assert!(!results.is_empty(), "should find at least one result");
        assert!(results[0].score > 0);
        assert!(!results[0].matched_lines.is_empty());
    }

    #[test]
    fn search_ignores_other_projects() {
        let dir = tempdir().unwrap();
        // other-proj memory
        let other_dir = dir.path().join(".gwenland/agent/memory/other-proj/conv1");
        fs::create_dir_all(&other_dir).unwrap();
        fs::write(other_dir.join("note.md"), "# Other\n- needle\n").unwrap();

        let results = search_memory(
            dir.path(),
            "proj",
            &["needle".into()],
            MemoryBudget::default(),
        )
        .unwrap();
        assert!(results.is_empty(), "must not cross project boundaries");
    }

    #[test]
    fn search_includes_all_conversations_for_project() {
        let dir = tempdir().unwrap();
        let c1 = dir.path().join(".gwenland/agent/memory/proj/conv1");
        let c2 = dir.path().join(".gwenland/agent/memory/proj/conv2");
        fs::create_dir_all(&c1).unwrap();
        fs::create_dir_all(&c2).unwrap();
        fs::write(c1.join("note1.md"), "# Note1\n- needle in conv1\n").unwrap();
        fs::write(c2.join("note2.md"), "# Note2\n- needle in conv2\n").unwrap();

        let results = search_memory(
            dir.path(),
            "proj",
            &["needle".into()],
            MemoryBudget::default(),
        )
        .unwrap();
        assert_eq!(
            results.len(),
            2,
            "should find notes from both conversations"
        );
    }

    #[test]
    fn search_scores_filename_hit_higher() {
        let dir = tempdir().unwrap();
        let mem_dir = dir.path().join(".gwenland/agent/memory/proj/conv1");
        fs::create_dir_all(&mem_dir).unwrap();
        fs::write(
            mem_dir.join("needle-fix.md"),
            "# Needle Fix\n- needle in filename\n",
        )
        .unwrap();
        fs::write(
            mem_dir.join("other-note.md"),
            "# Other\n- needle only in content\n",
        )
        .unwrap();

        let results = search_memory(
            dir.path(),
            "proj",
            &["needle".into()],
            MemoryBudget::default(),
        )
        .unwrap();
        assert!(results.len() >= 2);
        assert!(
            results[0].filename.contains("needle-fix"),
            "filename hit should score higher"
        );
    }

    // --- render_memory_block ---

    #[test]
    fn render_returns_none_for_empty_results() {
        assert!(render_memory_block(&[], MemoryBudget::default()).is_none());
    }

    #[test]
    fn render_wraps_in_memory_tags() {
        let results = vec![MemorySearchResult {
            filename: ".gwenland/agent/memory/proj/conv1/fix-null.md".into(),
            matched_lines: vec!["2: unwrap at line 42".into()],
            score: 5,
        }];
        let block = render_memory_block(&results, MemoryBudget::default()).unwrap();
        assert!(block.starts_with("<memory>"));
        assert!(block.ends_with("</memory>"));
        assert!(block.contains("[fix-null.md]"));
        assert!(block.contains("unwrap"));
    }

    #[test]
    fn render_caps_output() {
        let tiny_budget = MemoryBudget {
            max_chars: 50,
            max_lines_per_file: 2,
        };
        let results = vec![
            MemorySearchResult {
                filename: "note1.md".into(),
                matched_lines: vec!["1: lots of content here".into()],
                score: 10,
            },
            MemorySearchResult {
                filename: "note2.md".into(),
                matched_lines: vec!["1: more content here".into()],
                score: 5,
            },
        ];
        // With a tiny budget, some results may be dropped.
        let block = render_memory_block(&results, tiny_budget);
        if let Some(b) = &block {
            assert!(
                b.len() <= 200,
                "should be within reasonable size: {}",
                b.len()
            );
        }
    }

    // --- write_memory_note ---

    #[test]
    fn write_creates_new_note() {
        let dir = tempdir().unwrap();
        let target = MemoryWriteTarget {
            project_name: "proj".into(),
            conversation_name: "conv1".into(),
            filename: "fix-null.md".into(),
        };
        let note = MemoryNote {
            filename: "fix-null.md".into(),
            content: "# Fix Null\n- Root cause: unwrap\n".into(),
        };
        let path = write_memory_note(dir.path(), &target, &note).unwrap();
        assert!(path.is_file());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Fix Null"));
    }

    #[test]
    fn write_appends_with_separator() {
        let dir = tempdir().unwrap();
        let target = MemoryWriteTarget {
            project_name: "proj".into(),
            conversation_name: "conv1".into(),
            filename: "note.md".into(),
        };
        let note1 = MemoryNote {
            filename: "note.md".into(),
            content: "# First\n- content\n".into(),
        };
        let note2 = MemoryNote {
            filename: "note.md".into(),
            content: "# Second\n- more content\n".into(),
        };
        write_memory_note(dir.path(), &target, &note1).unwrap();
        write_memory_note(dir.path(), &target, &note2).unwrap();

        let path = memory_conversation_dir(dir.path(), "proj", "conv1").join("note.md");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("First"));
        assert!(content.contains("Second"));
        assert!(content.contains("---"));
    }

    #[test]
    fn write_rejects_empty_content() {
        let dir = tempdir().unwrap();
        let target = MemoryWriteTarget {
            project_name: "proj".into(),
            conversation_name: "conv1".into(),
            filename: "note.md".into(),
        };
        let note = MemoryNote {
            filename: "note.md".into(),
            content: "   \n\n  ".into(),
        };
        assert!(write_memory_note(dir.path(), &target, &note).is_err());
    }

    // --- JSON parsing ---

    #[test]
    fn parse_keyword_array_valid_json() {
        let raw = r#"["fix", "null", "parser"]"#;
        let kws = parse_keyword_array(raw);
        assert_eq!(kws, vec!["fix", "null", "parser"]);
    }

    #[test]
    fn parse_keyword_array_strips_fences() {
        let raw = "```json\n[\"fix\", \"null\"]\n```";
        let kws = parse_keyword_array(raw);
        assert_eq!(kws, vec!["fix", "null"]);
    }

    #[test]
    fn parse_keyword_array_invalid_returns_empty() {
        assert!(parse_keyword_array("not json at all").is_empty());
        assert!(parse_keyword_array("{}").is_empty());
        assert!(parse_keyword_array("").is_empty());
    }

    #[test]
    fn parse_keyword_array_drops_non_strings() {
        let raw = r#"["fix", 42, null, "parser"]"#;
        let kws = parse_keyword_array(raw);
        assert_eq!(kws, vec!["fix", "parser"]);
    }

    #[test]
    fn parse_memory_note_valid() {
        let raw = "{\"filename\": \"fix-null.md\", \"content\": \"# Fix\\n- root cause\\n\"}";
        let note = parse_memory_note(raw).unwrap();
        assert_eq!(note.filename, "fix-null.md");
        assert!(note.content.contains("Fix"));
    }

    #[test]
    fn parse_memory_note_strips_fences() {
        let raw = "```json\n{\"filename\": \"note.md\", \"content\": \"# Note - thing\"}\n```";
        let note = parse_memory_note(raw).unwrap();
        assert_eq!(note.filename, "note.md");
    }

    #[test]
    fn parse_memory_note_rejects_empty_content() {
        let raw = "{\"filename\": \"note.md\", \"content\": \"   \"}";
        assert!(parse_memory_note(raw).is_err());
    }

    #[test]
    fn parse_memory_note_rejects_missing_filename() {
        let raw = "{\"content\": \"# Fix - thing\"}";
        assert!(parse_memory_note(raw).is_err());
    }
}
