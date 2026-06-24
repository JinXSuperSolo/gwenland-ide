//! Agent tools (M10 Wave 7).
//!
//! Provider-neutral tool model for the ReAct loop, plus pure executors for the
//! read-only tools that need nothing but the filesystem + safety policy. Tools
//! with side effects or process/IPC needs (git diff, diagnostics, terminal,
//! browser, ask-user) and the mutating tools (which flow through the Apply Gate)
//! are dispatched by the Tauri layer — the engine only defines their contracts.
//!
//! No Tauri/UI here. Tool calls are exchanged as JSON so the loop is provider-
//! agnostic (a text protocol the system prompt teaches the model — see
//! `agent_loop::parse_tool_call`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agentic::context::MAX_ITEM_BYTES;
use crate::agentic::policy::{is_excluded_path, is_secret_path};

/// What kind of side effect a tool has — drives which gate the runtime applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSide {
    /// No side effects; runs immediately.
    Read,
    /// Writes files; routes through the Apply Gate (reviewable ChangeSet).
    Mutating,
    /// Runs a shell command; routes through the Validation Gate.
    Terminal,
    /// Interacts with the user / opens a browser; needs the UI.
    Interaction,
}

/// Every tool the agent can call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    ReadFile,
    ListDir,
    GrepSearch,
    FileSearch,
    GetGitDiff,
    GetDiagnostics,
    EditFile,
    WriteFile,
    DeleteFile,
    RunTerminalCmd,
    AskUser,
    OpenBrowser,
}

/// All tools, in advertised order.
pub const ALL_TOOLS: &[ToolKind] = &[
    ToolKind::ReadFile,
    ToolKind::ListDir,
    ToolKind::GrepSearch,
    ToolKind::FileSearch,
    ToolKind::GetGitDiff,
    ToolKind::GetDiagnostics,
    ToolKind::EditFile,
    ToolKind::WriteFile,
    ToolKind::DeleteFile,
    ToolKind::RunTerminalCmd,
    ToolKind::AskUser,
    ToolKind::OpenBrowser,
];

/// A tool's advertised contract for the system prompt.
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// Short args description, e.g. `{"path": string}`.
    pub args: &'static str,
}

impl ToolKind {
    /// Canonical snake_case name used in the JSON tool protocol.
    pub fn name(self) -> &'static str {
        match self {
            ToolKind::ReadFile => "read_file",
            ToolKind::ListDir => "list_dir",
            ToolKind::GrepSearch => "grep_search",
            ToolKind::FileSearch => "file_search",
            ToolKind::GetGitDiff => "get_git_diff",
            ToolKind::GetDiagnostics => "get_diagnostics",
            ToolKind::EditFile => "edit_file",
            ToolKind::WriteFile => "write_file",
            ToolKind::DeleteFile => "delete_file",
            ToolKind::RunTerminalCmd => "run_terminal_cmd",
            ToolKind::AskUser => "ask_user",
            ToolKind::OpenBrowser => "open_browser",
        }
    }

    /// Parse a tool name from the model's JSON.
    pub fn from_name(name: &str) -> Option<ToolKind> {
        ALL_TOOLS.iter().copied().find(|t| t.name() == name)
    }

    /// The tool's side-effect class.
    pub fn side(self) -> ToolSide {
        match self {
            ToolKind::ReadFile
            | ToolKind::ListDir
            | ToolKind::GrepSearch
            | ToolKind::FileSearch
            | ToolKind::GetGitDiff
            | ToolKind::GetDiagnostics => ToolSide::Read,
            ToolKind::EditFile | ToolKind::WriteFile | ToolKind::DeleteFile => ToolSide::Mutating,
            ToolKind::RunTerminalCmd => ToolSide::Terminal,
            ToolKind::AskUser | ToolKind::OpenBrowser => ToolSide::Interaction,
        }
    }

    /// True when the engine can execute this tool with only fs + policy
    /// ([`execute_local_tool`]); the rest are dispatched by the Tauri layer.
    pub fn is_engine_local(self) -> bool {
        matches!(
            self,
            ToolKind::ReadFile | ToolKind::ListDir | ToolKind::GrepSearch | ToolKind::FileSearch
        )
    }

    pub fn spec(self) -> ToolSpec {
        match self {
            ToolKind::ReadFile => ToolSpec {
                name: "read_file",
                description: "Read a UTF-8 text file in the workspace.",
                args: r#"{"path": string}"#,
            },
            ToolKind::ListDir => ToolSpec {
                name: "list_dir",
                description: "List entries of a workspace directory.",
                args: r#"{"path": string (default ".")}"#,
            },
            ToolKind::GrepSearch => ToolSpec {
                name: "grep_search",
                description: "Search file contents for a substring (case-insensitive).",
                args: r#"{"query": string}"#,
            },
            ToolKind::FileSearch => ToolSpec {
                name: "file_search",
                description: "Find files whose name contains the query.",
                args: r#"{"query": string}"#,
            },
            ToolKind::GetGitDiff => ToolSpec {
                name: "get_git_diff",
                description: "Show the current git diff for the workspace.",
                args: r#"{}"#,
            },
            ToolKind::GetDiagnostics => ToolSpec {
                name: "get_diagnostics",
                description: "List current LSP diagnostics (optionally for one file).",
                args: r#"{"path": string (optional)}"#,
            },
            ToolKind::EditFile => ToolSpec {
                name: "edit_file",
                description: "Propose a unified-diff edit to a file (Apply Gate — needs approval).",
                args: r#"{"path": string, "diff": string}"#,
            },
            ToolKind::WriteFile => ToolSpec {
                name: "write_file",
                description: "Propose creating/replacing a file (Apply Gate — needs approval).",
                args: r#"{"path": string, "content": string}"#,
            },
            ToolKind::DeleteFile => ToolSpec {
                name: "delete_file",
                description: "Propose deleting a file (Apply Gate — needs explicit confirmation).",
                args: r#"{"path": string}"#,
            },
            ToolKind::RunTerminalCmd => ToolSpec {
                name: "run_terminal_cmd",
                description: "Propose a terminal command (Validation Gate — needs approval).",
                args: r#"{"command": string, "reason": string}"#,
            },
            ToolKind::AskUser => ToolSpec {
                name: "ask_user",
                description: "Ask the user to choose one or more options.",
                args: r#"{"prompt": string, "options": string[], "multi": bool}"#,
            },
            ToolKind::OpenBrowser => ToolSpec {
                name: "open_browser",
                description: "Open an http/https URL in the user's browser.",
                args: r#"{"url": string}"#,
            },
        }
    }
}

/// A tool invocation parsed from the model's output. `id` is engine-minted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub tool: ToolKind,
    pub args: Value,
}

/// The observation fed back into the loop after a tool runs (or is rejected).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub id: String,
    pub ok: bool,
    pub content: String,
    pub error: Option<String>,
}

impl ToolResult {
    pub fn ok(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ok: true,
            content: content.into(),
            error: None,
        }
    }
    pub fn err(id: impl Into<String>, error: impl Into<String>) -> Self {
        let error = error.into();
        Self {
            id: id.into(),
            ok: false,
            content: String::new(),
            error: Some(error),
        }
    }
}

/// Render the tool list for the system/user prompt.
pub fn render_tool_specs() -> String {
    let mut out = String::new();
    for tool in ALL_TOOLS {
        let s = tool.spec();
        out.push_str(&format!("- {} {} — {}\n", s.name, s.args, s.description));
    }
    out
}

// --- Pure read-tool execution ----------------------------------------------

/// Per-search/scan caps so a tool call can never run away on a big repo.
const MAX_WALK_FILES: usize = 4000;
const MAX_MATCHES: usize = 100;
const MAX_LISTING: usize = 300;

fn arg_str(call: &ToolCall, key: &str) -> Option<String> {
    call.args
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn truncate_bytes(s: String, max: usize) -> (String, bool) {
    if s.len() <= max {
        return (s, false);
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    (s[..end].to_string(), true)
}

/// Canonicalize a read target and confirm it is the workspace root or inside it.
/// Unlike `policy::canonical_within_workspace` (which is for not-yet-existing edit
/// targets and excludes the root), this allows the root itself so `list_dir(".")`
/// works, and requires the path to already exist.
fn read_canonical(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let root_c = root
        .canonicalize()
        .map_err(|_| "workspace root cannot be resolved".to_string())?;
    let target = root
        .join(rel)
        .canonicalize()
        .map_err(|_| format!("path not found: {rel}"))?;
    if target == root_c || target.starts_with(&root_c) {
        Ok(target)
    } else {
        Err("path is outside the workspace".to_string())
    }
}

/// Execute a read-only tool the engine can run itself (read_file, list_dir,
/// grep_search, file_search). Returns `None` for tools the Tauri layer handles.
pub fn execute_local_tool(root: &Path, call: &ToolCall) -> Option<ToolResult> {
    match call.tool {
        ToolKind::ReadFile => Some(exec_read_file(root, call)),
        ToolKind::ListDir => Some(exec_list_dir(root, call)),
        ToolKind::GrepSearch => Some(exec_grep(root, call)),
        ToolKind::FileSearch => Some(exec_file_search(root, call)),
        _ => None,
    }
}

fn exec_read_file(root: &Path, call: &ToolCall) -> ToolResult {
    let path = match arg_str(call, "path") {
        Some(p) if !p.trim().is_empty() => p,
        _ => return ToolResult::err(&call.id, "missing required arg 'path'"),
    };
    if is_secret_path(&path) {
        return ToolResult::err(&call.id, "refused: path matches a secret pattern");
    }
    let canon = match read_canonical(root, &path) {
        Ok(c) => c,
        Err(e) => return ToolResult::err(&call.id, format!("refused: {e}")),
    };
    match crate::fs::read_file(&canon) {
        Ok(text) => {
            let (text, truncated) = truncate_bytes(text, MAX_ITEM_BYTES);
            let suffix = if truncated { "\n…(truncated)" } else { "" };
            ToolResult::ok(&call.id, format!("{path}\n{text}{suffix}"))
        }
        Err(e) => ToolResult::err(&call.id, format!("read failed: {e}")),
    }
}

fn exec_list_dir(root: &Path, call: &ToolCall) -> ToolResult {
    let path = arg_str(call, "path")
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| ".".into());
    let canon = match read_canonical(root, &path) {
        Ok(c) => c,
        Err(e) => return ToolResult::err(&call.id, format!("refused: {e}")),
    };
    match crate::fs::list_directory(&canon) {
        Ok(entries) => {
            let mut lines = Vec::new();
            for e in entries {
                if is_excluded_path(&e.name) || is_secret_path(&e.name) {
                    continue;
                }
                lines.push(if e.is_dir {
                    format!("{}/", e.name)
                } else {
                    e.name
                });
                if lines.len() >= MAX_LISTING {
                    lines.push("…(more)".into());
                    break;
                }
            }
            let body = if lines.is_empty() {
                "(empty)".into()
            } else {
                lines.join("\n")
            };
            ToolResult::ok(&call.id, format!("{path}\n{body}"))
        }
        Err(e) => ToolResult::err(&call.id, format!("list failed: {e}")),
    }
}

fn exec_grep(root: &Path, call: &ToolCall) -> ToolResult {
    let query = match arg_str(call, "query") {
        Some(q) if !q.trim().is_empty() => q,
        _ => return ToolResult::err(&call.id, "missing required arg 'query'"),
    };
    let needle = query.to_ascii_lowercase();
    let mut matches: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    for_each_file(root, root, &mut scanned, &mut |rel, abs| {
        let content = match std::fs::read_to_string(abs) {
            Ok(c) => c,
            Err(_) => return true, // skip binary/unreadable
        };
        for (i, line) in content.lines().enumerate() {
            if line.to_ascii_lowercase().contains(&needle) {
                let (snippet, _) = truncate_bytes(line.trim().to_string(), 200);
                matches.push(format!("{rel}:{}: {snippet}", i + 1));
                if matches.len() >= MAX_MATCHES {
                    return false;
                }
            }
        }
        true
    });
    let body = if matches.is_empty() {
        format!("No matches for '{query}'.")
    } else {
        matches.join("\n")
    };
    ToolResult::ok(&call.id, body)
}

fn exec_file_search(root: &Path, call: &ToolCall) -> ToolResult {
    let query = match arg_str(call, "query") {
        Some(q) if !q.trim().is_empty() => q,
        _ => return ToolResult::err(&call.id, "missing required arg 'query'"),
    };
    let needle = query.to_ascii_lowercase();
    let mut hits: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    for_each_file(root, root, &mut scanned, &mut |rel, _abs| {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        if name.to_ascii_lowercase().contains(&needle) {
            hits.push(rel.to_string());
            if hits.len() >= MAX_LISTING {
                return false;
            }
        }
        true
    });
    let body = if hits.is_empty() {
        format!("No files matching '{query}'.")
    } else {
        hits.join("\n")
    };
    ToolResult::ok(&call.id, body)
}

/// Depth-first walk of workspace files, skipping excluded/secret dirs and files
/// larger than the per-item budget. `visit(rel, abs)` returns false to stop.
fn for_each_file(
    root: &Path,
    dir: &Path,
    scanned: &mut usize,
    visit: &mut impl FnMut(&str, &Path) -> bool,
) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return true,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if is_excluded_path(&name) || is_secret_path(&name) {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let path = entry.path();
        if file_type.is_dir() {
            if !for_each_file(root, &path, scanned, visit) {
                return false;
            }
        } else if file_type.is_file() {
            *scanned += 1;
            if *scanned > MAX_WALK_FILES {
                return false;
            }
            if let Ok(meta) = entry.metadata() {
                if meta.len() > MAX_ITEM_BYTES as u64 {
                    continue;
                }
            }
            let rel = path
                .strip_prefix(root)
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

// --- Mutation path resolution (M10) ----------------------------------------

/// Outcome of resolving a model-supplied file path to a real workspace file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathResolution {
    /// The path already exists (workspace-relative form returned).
    Exact(String),
    /// The path was missing, but exactly one file with that basename exists.
    Corrected(String),
    /// Several files share the basename; the caller must disambiguate.
    Ambiguous(Vec<String>),
    /// No file with that basename exists in the workspace.
    NotFound,
}

/// Resolve `rel` against the workspace: `Exact` if it already exists, otherwise
/// search by basename (case-insensitive, the same walk `file_search` uses) for an
/// existing file, yielding `Corrected` (one match), `Ambiguous` (several), or
/// `NotFound`. Secret/excluded paths are never matched.
pub fn resolve_workspace_file(root: &Path, rel: &str) -> PathResolution {
    let rel_norm = rel.replace('\\', "/");
    if !is_secret_path(&rel_norm)
        && read_canonical(root, &rel_norm).is_ok()
        && root.join(&rel_norm).is_file()
    {
        return PathResolution::Exact(rel_norm);
    }
    let base = rel_norm
        .rsplit('/')
        .next()
        .unwrap_or(&rel_norm)
        .to_ascii_lowercase();
    if base.is_empty() {
        return PathResolution::NotFound;
    }
    let mut hits: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    for_each_file(root, root, &mut scanned, &mut |rel_found, _abs| {
        let name = rel_found
            .rsplit('/')
            .next()
            .unwrap_or(rel_found)
            .to_ascii_lowercase();
        if name == base && !is_secret_path(rel_found) {
            hits.push(rel_found.to_string());
            if hits.len() > 8 {
                return false;
            }
        }
        true
    });
    match hits.len() {
        0 => PathResolution::NotFound,
        1 => PathResolution::Corrected(hits.into_iter().next().unwrap()),
        _ => PathResolution::Ambiguous(hits),
    }
}

/// What the runtime should do after pre-checking a mutating tool's target path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationPreflight {
    /// Proceed to the gate with the call unchanged.
    Proceed,
    /// Proceed; the call's `path` arg was rewritten to this resolved path.
    Corrected(String),
    /// Do not gate; feed this message back to the model so it self-corrects.
    Reject(String),
}

/// Before gating an `edit_file`/`delete_file`, ensure its target exists. If the
/// given path is missing but a single same-named file exists, rewrite the call's
/// `path` to it (`Corrected`); if several or none match, `Reject` with guidance
/// to call `file_search`. `write_file` (which may create) and non-mutating tools
/// always `Proceed`. This keeps a doomed write from ever reaching the Apply Gate
/// (e.g. the OS "path not found" that happens when the model guesses a path).
pub fn preflight_mutation_path(root: &Path, call: &mut ToolCall) -> MutationPreflight {
    if !matches!(call.tool, ToolKind::EditFile | ToolKind::DeleteFile) {
        return MutationPreflight::Proceed;
    }
    let path = match call.args.get("path").and_then(|v| v.as_str()) {
        Some(p) if !p.trim().is_empty() => p.to_string(),
        // A missing path arg is reported by the apply step itself.
        _ => return MutationPreflight::Proceed,
    };
    match resolve_workspace_file(root, &path) {
        PathResolution::Exact(_) => MutationPreflight::Proceed,
        PathResolution::Corrected(found) => {
            if let Some(obj) = call.args.as_object_mut() {
                obj.insert("path".to_string(), Value::String(found.clone()));
            }
            MutationPreflight::Corrected(found)
        }
        PathResolution::Ambiguous(hits) => MutationPreflight::Reject(format!(
            "'{path}' was not found. Several files share that name — call file_search and use the correct path before editing:\n{}",
            hits.join("\n")
        )),
        PathResolution::NotFound => MutationPreflight::Reject(format!(
            "'{path}' does not exist in the workspace. Call file_search to find the correct path before editing."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    fn call(tool: ToolKind, args: Value) -> ToolCall {
        ToolCall {
            id: "t1".into(),
            tool,
            args,
        }
    }

    #[test]
    fn names_round_trip() {
        for t in ALL_TOOLS {
            assert_eq!(ToolKind::from_name(t.name()), Some(*t));
        }
        assert_eq!(ToolKind::from_name("nope"), None);
    }

    #[test]
    fn sides_are_classified() {
        assert_eq!(ToolKind::ReadFile.side(), ToolSide::Read);
        assert_eq!(ToolKind::EditFile.side(), ToolSide::Mutating);
        assert_eq!(ToolKind::RunTerminalCmd.side(), ToolSide::Terminal);
        assert_eq!(ToolKind::OpenBrowser.side(), ToolSide::Interaction);
    }

    #[test]
    fn read_file_reads_within_workspace() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello world").unwrap();
        let res = execute_local_tool(
            dir.path(),
            &call(ToolKind::ReadFile, json!({"path": "a.txt"})),
        )
        .unwrap();
        assert!(res.ok);
        assert!(res.content.contains("hello world"));
    }

    #[test]
    fn read_file_rejects_secret_and_outside() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".env"), "SECRET=1").unwrap();
        let secret = execute_local_tool(
            dir.path(),
            &call(ToolKind::ReadFile, json!({"path": ".env"})),
        )
        .unwrap();
        assert!(!secret.ok);

        let outside = execute_local_tool(
            dir.path(),
            &call(ToolKind::ReadFile, json!({"path": "../escape.txt"})),
        )
        .unwrap();
        assert!(!outside.ok);
    }

    #[test]
    fn list_dir_skips_excluded_and_secret() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join(".env"), "x").unwrap();
        fs::write(dir.path().join("readme.md"), "x").unwrap();
        let res =
            execute_local_tool(dir.path(), &call(ToolKind::ListDir, json!({"path": "."}))).unwrap();
        assert!(res.ok);
        assert!(res.content.contains("src/"));
        assert!(res.content.contains("readme.md"));
        assert!(!res.content.contains("node_modules"));
        assert!(!res.content.contains(".env"));
    }

    #[test]
    fn grep_finds_matches_and_skips_excluded() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "fn needle() {}\nother\n").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target/b.rs"), "needle in build output").unwrap();
        let res = execute_local_tool(
            dir.path(),
            &call(ToolKind::GrepSearch, json!({"query": "needle"})),
        )
        .unwrap();
        assert!(res.ok);
        assert!(res.content.contains("a.rs:1"));
        assert!(!res.content.contains("target")); // excluded dir not scanned
    }

    #[test]
    fn file_search_matches_names() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/widget.ts"), "x").unwrap();
        let res = execute_local_tool(
            dir.path(),
            &call(ToolKind::FileSearch, json!({"query": "widget"})),
        )
        .unwrap();
        assert!(res.ok);
        assert!(res.content.contains("src/widget.ts"));
    }

    #[test]
    fn non_local_tools_return_none() {
        let dir = tempdir().unwrap();
        assert!(
            execute_local_tool(dir.path(), &call(ToolKind::RunTerminalCmd, json!({}))).is_none()
        );
        assert!(execute_local_tool(dir.path(), &call(ToolKind::EditFile, json!({}))).is_none());
    }

    #[test]
    fn resolve_finds_exact_corrected_and_missing() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/lib/components")).unwrap();
        fs::write(dir.path().join("src/lib/components/Layout.tsx"), "x").unwrap();

        // Exact match (the path as given exists).
        assert_eq!(
            resolve_workspace_file(dir.path(), "src/lib/components/Layout.tsx"),
            PathResolution::Exact("src/lib/components/Layout.tsx".into())
        );
        // Wrong directory, unique basename → corrected to the real path.
        assert_eq!(
            resolve_workspace_file(dir.path(), "src/components/Layout.tsx"),
            PathResolution::Corrected("src/lib/components/Layout.tsx".into())
        );
        // No such file anywhere.
        assert_eq!(
            resolve_workspace_file(dir.path(), "src/Nope.tsx"),
            PathResolution::NotFound
        );
    }

    #[test]
    fn resolve_reports_ambiguous() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a")).unwrap();
        fs::create_dir_all(dir.path().join("b")).unwrap();
        fs::write(dir.path().join("a/Layout.tsx"), "x").unwrap();
        fs::write(dir.path().join("b/Layout.tsx"), "x").unwrap();
        match resolve_workspace_file(dir.path(), "src/Layout.tsx") {
            PathResolution::Ambiguous(hits) => assert_eq!(hits.len(), 2),
            other => panic!("expected ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn preflight_corrects_edit_path_in_place() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/lib/components")).unwrap();
        fs::write(dir.path().join("src/lib/components/Layout.tsx"), "x").unwrap();

        let mut c = call(
            ToolKind::EditFile,
            json!({"path": "src/components/Layout.tsx", "diff": "..."}),
        );
        let outcome = preflight_mutation_path(dir.path(), &mut c);
        assert_eq!(
            outcome,
            MutationPreflight::Corrected("src/lib/components/Layout.tsx".into())
        );
        // The call now carries the corrected path for the Apply Gate.
        assert_eq!(
            c.args.get("path").and_then(|v| v.as_str()),
            Some("src/lib/components/Layout.tsx")
        );
    }

    #[test]
    fn preflight_rejects_missing_edit_target() {
        let dir = tempdir().unwrap();
        let mut c = call(
            ToolKind::EditFile,
            json!({"path": "src/Ghost.tsx", "diff": "..."}),
        );
        match preflight_mutation_path(dir.path(), &mut c) {
            MutationPreflight::Reject(msg) => assert!(msg.contains("file_search")),
            other => panic!("expected reject, got {other:?}"),
        }
    }

    #[test]
    fn preflight_proceeds_for_write_file() {
        // write_file may create a new file, so a missing path must not be rejected.
        let dir = tempdir().unwrap();
        let mut c = call(
            ToolKind::WriteFile,
            json!({"path": "src/new/File.ts", "content": "x"}),
        );
        assert_eq!(
            preflight_mutation_path(dir.path(), &mut c),
            MutationPreflight::Proceed
        );
    }
}
