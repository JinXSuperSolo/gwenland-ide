//! Git integration (Milestone "Wave 2" — GWEN-327..331).
//!
//! A thin, dependency-free wrapper over the system `git` binary (`std::process`
//! only — no git2/libgit2, to respect the binary budget). Every operation is
//! scoped to a workspace root passed by the caller; nothing here touches global
//! git config. All functions return a normalized [`GitError`] on failure.
//!
//! The frontend never shells out itself — it calls the `git_*` Tauri commands,
//! which delegate here.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use serde::Serialize;
use thiserror::Error;

const DEFAULT_GRAPH_LIMIT: usize = 300;
const MAX_GRAPH_LIMIT: usize = 500;
const GRAPH_LANE_HEIGHT: i32 = 32;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("not a git repository")]
    NotARepo,
    #[error("git is not installed or not on PATH")]
    GitMissing,
    #[error("git command failed: {0}")]
    CommandFailed(String),
    #[error("io error: {0}")]
    Io(String),
}

fn hide_child_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

/// A single changed file from `git status --porcelain` (GWEN-328/329).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GitFileStatus {
    /// Repo-relative path (forward slashes), already unquoted.
    pub path: String,
    /// Single-letter status badge for the UI: M/A/D/U/R/C.
    pub status: String,
    /// True when the change (or part of it) is staged in the index.
    pub staged: bool,
    /// True for an untracked file (`??`).
    pub untracked: bool,
}

/// Branch + dirty summary for the status bar (GWEN-327).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GitStatus {
    /// Current branch (or a short detached-HEAD label).
    pub branch: String,
    /// Count of changed/untracked entries (porcelain lines).
    pub dirty_count: usize,
    /// Per-file status list for the Git panel.
    pub files: Vec<GitFileStatus>,
    /// Commits ahead of the upstream tracking branch (0 when no upstream).
    pub ahead: usize,
    /// Commits behind the upstream tracking branch (0 when no upstream).
    pub behind: usize,
}

/// Run `git <args>` in `root`, returning stdout on success. Maps a missing
/// binary, a non-repo, and any other non-zero exit to a typed error.
fn run_git(root: &Path, args: &[&str]) -> Result<String, GitError> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(root);
    hide_child_window(&mut cmd);
    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            GitError::GitMissing
        } else {
            GitError::Io(e.to_string())
        }
    })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lower = stderr.to_lowercase();
        if lower.contains("not a git repository") {
            Err(GitError::NotARepo)
        } else {
            Err(GitError::CommandFailed(stderr.trim().to_string()))
        }
    }
}

/// Whether `root` is inside a git work tree. Cheap; used to hide all git UI when
/// false (GWEN-327/329).
pub fn is_git_repo(root: &Path) -> bool {
    matches!(
        run_git(root, &["rev-parse", "--is-inside-work-tree"]),
        Ok(out) if out.trim() == "true"
    )
}

/// Current branch name, or a short detached-HEAD label like `detached@1a2b3c4`.
pub fn current_branch(root: &Path) -> Result<String, GitError> {
    let name = run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string();
    if name == "HEAD" {
        // Detached HEAD — show the short SHA so the status bar isn't blank.
        let sha = run_git(root, &["rev-parse", "--short", "HEAD"])
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        Ok(format!("detached@{sha}"))
    } else {
        Ok(name)
    }
}

/// Unquote a porcelain path. Git quotes paths containing special chars in
/// double quotes with C-style escapes; the common cases are spaces (not quoted)
/// and quoted unicode. We only strip the surrounding quotes + unescape `\"`/`\\`.
fn unquote_path(raw: &str) -> String {
    let s = raw.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        s.to_string()
    }
    .replace('\\', "/")
}

/// Parse `git status --porcelain` output into per-file entries. The porcelain
/// format is `XY <path>` where X is the index (staged) state and Y the worktree
/// state. Rename lines (`R  old -> new`) keep the new path.
fn parse_porcelain(out: &str) -> Vec<GitFileStatus> {
    let mut files = Vec::new();
    for line in out.lines() {
        if line.len() < 3 {
            continue;
        }
        let x = line.as_bytes()[0] as char;
        let y = line.as_bytes()[1] as char;
        let rest = &line[3..];

        let untracked = x == '?' && y == '?';
        // Renames/copies show "old -> new"; record the destination path.
        let path_part = if let Some(idx) = rest.find(" -> ") {
            &rest[idx + 4..]
        } else {
            rest
        };
        let path = unquote_path(path_part);

        // A change is staged iff the index column is a real status letter.
        let staged = !untracked && x != ' ' && x != '?';
        // Pick the most meaningful single-letter badge for the UI.
        let status = if untracked {
            'U'
        } else if x != ' ' && x != '?' {
            x
        } else {
            y
        };

        files.push(GitFileStatus {
            path,
            status: status.to_string(),
            staged,
            untracked,
        });
    }
    files
}

/// Commits ahead/behind the upstream tracking branch. Returns `(ahead, behind)`.
/// Returns `(0, 0)` gracefully when the branch has no upstream or git fails.
pub fn ahead_behind(root: &Path) -> (usize, usize) {
    // `rev-list --count HEAD...@{u}` outputs "ahead\tbehind" with --left-right.
    let out = match run_git(
        root,
        &["rev-list", "--count", "--left-right", "HEAD...@{u}"],
    ) {
        Ok(s) => s,
        Err(_) => return (0, 0),
    };
    let mut parts = out.split_whitespace();
    let ahead = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let behind = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (ahead, behind)
}

/// Full status snapshot: branch + dirty count + per-file list (GWEN-327/328).
pub fn status(root: &Path) -> Result<GitStatus, GitError> {
    let branch = current_branch(root)?;
    let porcelain = run_git(root, &["status", "--porcelain"])?;
    let files = parse_porcelain(&porcelain);
    let (ahead, behind) = ahead_behind(root);
    Ok(GitStatus {
        dirty_count: files.len(),
        branch,
        files,
        ahead,
        behind,
    })
}

// --- Git Graph (GWEN-404 Phase 1) -----------------------------------------
//
// Migration plan:
// 1. Keep all history reads inside this existing dependency-free git module so
//    the restructured workspace domain remains the single owner of git access.
// 2. Add read-only graph DTOs, parser, first-parent lane assignment, and edge
//    precomputation here; the frontend only renders the payload and never shells
//    out or recomputes graph relationships per frame.
// 3. Expose `get_git_graph` first, then lazy `get_commit_details` /
//    `get_commit_diff` for Phase 2. Dock search/navigation, lane recycling, and
//    canvas labels stay out of this migration until later phases.
// 4. Continue using direct `std::process::Command::new("git")` with hidden
//    child windows on Windows, and keep the default history cap at 300 with an
//    enforced hard cap of 500.

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawCommit {
    hash: String,
    short_hash: String,
    subject: String,
    author: String,
    date: String,
    parents: Vec<String>,
    refs: Vec<String>,
    branch_refs: Vec<(String, bool)>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitNode {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
    pub relative_date: String,
    pub parents: Vec<String>,
    pub refs: Vec<String>,
    pub lane: usize,
    pub x: usize,
    pub y: i32,
    pub is_head: bool,
    pub is_merge: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommitEdgeKind {
    Linear,
    Fork,
    Merge,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitEdge {
    pub from: String,
    pub to: String,
    pub from_lane: usize,
    pub to_lane: usize,
    pub from_x: usize,
    pub to_x: usize,
    pub kind: CommitEdgeKind,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BranchRef {
    pub name: String,
    pub hash: String,
    pub is_remote: bool,
    pub lane: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitGraphPayload {
    pub nodes: Vec<CommitNode>,
    pub edges: Vec<CommitEdge>,
    pub branches: Vec<BranchRef>,
    pub head: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitFileChange {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetails {
    pub hash: String,
    pub full_message: String,
    pub author: String,
    pub date: String,
    pub files_changed: Vec<CommitFileChange>,
    pub insertions: usize,
    pub deletions: usize,
}

fn empty_graph_payload() -> CommitGraphPayload {
    CommitGraphPayload {
        nodes: Vec::new(),
        edges: Vec::new(),
        branches: Vec::new(),
        head: None,
        truncated: false,
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !value.is_empty() && !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn normalize_ref_display(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "HEAD" {
        return Some("HEAD".to_string());
    }

    let tag = trimmed.strip_prefix("tag: ");
    let bare = tag.unwrap_or(trimmed);
    if let Some(name) = bare.strip_prefix("refs/heads/") {
        Some(name.to_string())
    } else if let Some(name) = bare.strip_prefix("refs/remotes/") {
        Some(name.to_string())
    } else if let Some(name) = bare.strip_prefix("refs/tags/") {
        Some(format!("tag: {name}"))
    } else if tag.is_some() {
        Some(format!("tag: {bare}"))
    } else {
        Some(bare.to_string())
    }
}

fn branch_from_decoration(raw: &str) -> Option<(String, bool)> {
    let trimmed = raw
        .trim()
        .strip_prefix("HEAD -> ")
        .unwrap_or(raw.trim())
        .trim();
    if let Some(name) = trimmed.strip_prefix("refs/heads/") {
        Some((name.to_string(), false))
    } else if let Some(name) = trimmed.strip_prefix("refs/remotes/") {
        if name.ends_with("/HEAD") {
            None
        } else {
            Some((name.to_string(), true))
        }
    } else {
        None
    }
}

fn parse_ref_decorations(raw: &str) -> (Vec<String>, Vec<(String, bool)>) {
    let mut refs = Vec::new();
    let mut branches = Vec::new();
    let mut seen_branches = HashSet::new();

    for decoration in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(target) = decoration.strip_prefix("HEAD -> ") {
            push_unique(&mut refs, "HEAD".to_string());
            if let Some(display) = normalize_ref_display(target) {
                push_unique(&mut refs, display);
            }
            if let Some((name, is_remote)) = branch_from_decoration(target)
                && seen_branches.insert(name.clone())
            {
                branches.push((name, is_remote));
            }
            continue;
        }

        if let Some(display) = normalize_ref_display(decoration) {
            push_unique(&mut refs, display);
        }
        if let Some((name, is_remote)) = branch_from_decoration(decoration)
            && seen_branches.insert(name.clone())
        {
            branches.push((name, is_remote));
        }
    }

    (refs, branches)
}

fn parse_git_log_line(line: &str) -> Option<RawCommit> {
    let parts: Vec<&str> = line.splitn(7, '|').collect();
    if parts.len() < 7 {
        return None;
    }
    let (refs, branch_refs) = parse_ref_decorations(parts[6]);
    Some(RawCommit {
        hash: parts[0].to_string(),
        short_hash: parts[1].to_string(),
        subject: parts[2].to_string(),
        author: parts[3].to_string(),
        date: parts[4].to_string(),
        parents: parts[5].split_whitespace().map(str::to_string).collect(),
        refs,
        branch_refs,
    })
}

fn relative_date(iso_date: &str) -> String {
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    let Ok(date) = OffsetDateTime::parse(iso_date, &Rfc3339) else {
        return String::new();
    };
    let seconds = (OffsetDateTime::now_utc() - date).whole_seconds().max(0);
    match seconds {
        0..=59 => "just now".to_string(),
        60..=3_599 => {
            let minutes = seconds / 60;
            format!(
                "{minutes} minute{} ago",
                if minutes == 1 { "" } else { "s" }
            )
        }
        3_600..=86_399 => {
            let hours = seconds / 3_600;
            format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" })
        }
        86_400..=2_591_999 => {
            let days = seconds / 86_400;
            format!("{days} day{} ago", if days == 1 { "" } else { "s" })
        }
        2_592_000..=31_535_999 => {
            let months = seconds / 2_592_000;
            format!("{months} month{} ago", if months == 1 { "" } else { "s" })
        }
        _ => {
            let years = seconds / 31_536_000;
            format!("{years} year{} ago", if years == 1 { "" } else { "s" })
        }
    }
}

fn assign_lanes(commits: &[RawCommit]) -> Vec<CommitNode> {
    let by_hash: HashMap<&str, &RawCommit> = commits
        .iter()
        .map(|commit| (commit.hash.as_str(), commit))
        .collect();
    let mut lane_by_hash = HashMap::<String, usize>::new();

    if let Some(head) = commits.first() {
        let mut current_hash = Some(head.hash.as_str());
        while let Some(hash) = current_hash {
            if lane_by_hash.insert(hash.to_string(), 0).is_some() {
                break;
            }
            current_hash = by_hash
                .get(hash)
                .and_then(|commit| commit.parents.first().map(String::as_str));
        }
    }

    let mut next_lane = 1usize;
    for commit in commits {
        if lane_by_hash.contains_key(&commit.hash) {
            continue;
        }
        let lane = next_lane;
        next_lane += 1;

        let mut current_hash = commit.hash.as_str();
        loop {
            if lane_by_hash.contains_key(current_hash) {
                break;
            }
            lane_by_hash.insert(current_hash.to_string(), lane);
            let Some(parent) = by_hash
                .get(current_hash)
                .and_then(|candidate| candidate.parents.first())
            else {
                break;
            };
            if lane_by_hash.contains_key(parent) {
                break;
            }
            current_hash = parent;
        }
    }

    let total = commits.len();
    commits
        .iter()
        .enumerate()
        .map(|(index, commit)| {
            let lane = *lane_by_hash.get(&commit.hash).unwrap_or(&0);
            CommitNode {
                hash: commit.hash.clone(),
                short_hash: commit.short_hash.clone(),
                message: commit.subject.clone(),
                author: commit.author.clone(),
                date: commit.date.clone(),
                relative_date: relative_date(&commit.date),
                parents: commit.parents.clone(),
                refs: commit.refs.clone(),
                lane,
                x: total.saturating_sub(index + 1),
                y: lane as i32 * GRAPH_LANE_HEIGHT,
                is_head: index == 0 || commit.refs.iter().any(|r| r == "HEAD"),
                is_merge: commit.parents.len() > 1,
            }
        })
        .collect()
}

fn build_edges(nodes: &[CommitNode]) -> Vec<CommitEdge> {
    let by_hash: HashMap<&str, &CommitNode> = nodes
        .iter()
        .map(|node| (node.hash.as_str(), node))
        .collect();
    let mut edges = Vec::new();

    for node in nodes {
        for (parent_index, parent_hash) in node.parents.iter().enumerate() {
            let Some(parent) = by_hash.get(parent_hash.as_str()) else {
                continue;
            };
            let kind = if parent_index == 0 {
                if node.lane == parent.lane {
                    CommitEdgeKind::Linear
                } else {
                    CommitEdgeKind::Merge
                }
            } else {
                CommitEdgeKind::Fork
            };
            edges.push(CommitEdge {
                from: node.hash.clone(),
                to: parent.hash.clone(),
                from_lane: node.lane,
                to_lane: parent.lane,
                from_x: node.x,
                to_x: parent.x,
                kind,
            });
        }
    }

    edges
}

fn build_branch_refs(commits: &[RawCommit], nodes: &[CommitNode]) -> Vec<BranchRef> {
    let lane_by_hash: HashMap<&str, usize> = nodes
        .iter()
        .map(|node| (node.hash.as_str(), node.lane))
        .collect();
    let mut seen = HashSet::new();
    let mut branches = Vec::new();

    for commit in commits {
        for (name, is_remote) in &commit.branch_refs {
            if seen.insert(name.clone()) {
                branches.push(BranchRef {
                    name: name.clone(),
                    hash: commit.hash.clone(),
                    is_remote: *is_remote,
                    lane: *lane_by_hash.get(commit.hash.as_str()).unwrap_or(&0),
                });
            }
        }
    }

    branches
}

fn is_empty_history_error(error: &GitError) -> bool {
    let GitError::CommandFailed(message) = error else {
        return false;
    };
    let lower = message.to_lowercase();
    lower.contains("does not have any commits")
        || lower.contains("bad default revision")
        || lower.contains("unknown revision or path not in the working tree")
}

/// Read a bounded, precomputed commit graph for the canvas renderer.
pub fn graph(root: &Path, max_commits: Option<u32>) -> Result<CommitGraphPayload, GitError> {
    let limit =
        (max_commits.unwrap_or(DEFAULT_GRAPH_LIMIT as u32) as usize).clamp(1, MAX_GRAPH_LIMIT);
    let fetch_limit = limit.saturating_add(1).to_string();
    let output = match run_git(
        root,
        &[
            "log",
            "--topo-order",
            "--date=iso-strict",
            "--decorate=full",
            "--parents",
            "--pretty=format:%H|%h|%s|%an|%ai|%P|%D",
            "-n",
            &fetch_limit,
        ],
    ) {
        Ok(output) => output,
        Err(error) if is_empty_history_error(&error) => return Ok(empty_graph_payload()),
        Err(error) => return Err(error),
    };

    let mut commits: Vec<RawCommit> = output.lines().filter_map(parse_git_log_line).collect();
    let truncated = commits.len() > limit;
    if truncated {
        commits.truncate(limit);
    }

    let nodes = assign_lanes(&commits);
    let edges = build_edges(&nodes);
    let branches = build_branch_refs(&commits, &nodes);
    let head = nodes
        .iter()
        .find(|node| node.is_head)
        .or_else(|| nodes.first())
        .map(|node| node.hash.clone());

    Ok(CommitGraphPayload {
        nodes,
        edges,
        branches,
        head,
        truncated,
    })
}

fn validate_commit_hash(hash: &str) -> Result<&str, GitError> {
    let trimmed = hash.trim();
    if (7..=64).contains(&trimmed.len()) && trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(trimmed)
    } else {
        Err(GitError::CommandFailed("invalid commit hash".to_string()))
    }
}

fn parse_commit_metadata(out: &str) -> Result<(String, String, String, String), GitError> {
    let mut parts = out.splitn(4, '\x1f');
    let hash = parts.next().unwrap_or_default().trim().to_string();
    let author = parts.next().unwrap_or_default().trim().to_string();
    let date = parts.next().unwrap_or_default().trim().to_string();
    let full_message = parts
        .next()
        .unwrap_or_default()
        .trim_end_matches(['\r', '\n'])
        .to_string();

    if hash.is_empty() {
        Err(GitError::CommandFailed(
            "commit metadata was empty".to_string(),
        ))
    } else {
        Ok((hash, author, date, full_message))
    }
}

fn parse_name_status(out: &str) -> Vec<CommitFileChange> {
    out.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let raw_status = parts.next()?.trim();
            let first_path = parts.next()?.trim();
            if raw_status.is_empty() || first_path.is_empty() {
                return None;
            }
            let status = raw_status
                .chars()
                .next()
                .map(|ch| ch.to_string())
                .unwrap_or_default();
            let path = if matches!(status.as_str(), "R" | "C") {
                parts.next().unwrap_or(first_path)
            } else {
                first_path
            };
            Some(CommitFileChange {
                path: unquote_path(path),
                status,
            })
        })
        .collect()
}

fn parse_numstat(out: &str) -> (usize, usize) {
    out.lines().fold((0, 0), |(insertions, deletions), line| {
        let mut parts = line.splitn(3, '\t');
        let added = parts
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let removed = parts
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        (insertions + added, deletions + removed)
    })
}

/// Lazy, read-only metadata for one commit. The UI calls this only from the
/// click popup and caches the result by commit hash.
pub fn commit_details(root: &Path, hash: &str) -> Result<CommitDetails, GitError> {
    let hash = validate_commit_hash(hash)?;
    let metadata = run_git(
        root,
        &[
            "show",
            "--no-ext-diff",
            "--date=iso-strict",
            "--format=%H%x1f%an%x1f%ai%x1f%B",
            "--no-patch",
            hash,
        ],
    )?;
    let (hash, author, date, full_message) = parse_commit_metadata(&metadata)?;
    let files_changed = parse_name_status(&run_git(
        root,
        &["show", "--no-ext-diff", "--format=", "--name-status", &hash],
    )?);
    let (insertions, deletions) = parse_numstat(&run_git(
        root,
        &["show", "--no-ext-diff", "--format=", "--numstat", &hash],
    )?);

    Ok(CommitDetails {
        hash,
        full_message,
        author,
        date,
        files_changed,
        insertions,
        deletions,
    })
}

/// Lazy, read-only unified diff for a single commit.
pub fn commit_diff(root: &Path, hash: &str) -> Result<String, GitError> {
    let hash = validate_commit_hash(hash)?;
    run_git(
        root,
        &["show", "--no-ext-diff", "--format=", "--patch", hash],
    )
}

// --- Staging (GWEN-328) ----------------------------------------------------

/// Stage one path (`git add -- <path>`).
pub fn stage(root: &Path, path: &str) -> Result<(), GitError> {
    run_git(root, &["add", "--", path]).map(|_| ())
}

/// Stage everything (`git add -A`).
pub fn stage_all(root: &Path) -> Result<(), GitError> {
    run_git(root, &["add", "-A"]).map(|_| ())
}

/// Unstage one path (`git reset -q HEAD -- <path>`). Works on a repo with no
/// commits too (HEAD-less): falls back to removing it from the index.
pub fn unstage(root: &Path, path: &str) -> Result<(), GitError> {
    match run_git(root, &["reset", "-q", "HEAD", "--", path]) {
        Ok(_) => Ok(()),
        // No commits yet: there's no HEAD to reset against.
        Err(GitError::CommandFailed(_)) => {
            run_git(root, &["rm", "--cached", "-q", "--", path]).map(|_| ())
        }
        Err(e) => Err(e),
    }
}

/// Unstage everything (`git reset -q HEAD`).
pub fn unstage_all(root: &Path) -> Result<(), GitError> {
    match run_git(root, &["reset", "-q", "HEAD"]) {
        Ok(_) => Ok(()),
        Err(GitError::CommandFailed(_)) => {
            run_git(root, &["rm", "-r", "--cached", "-q", "."]).map(|_| ())
        }
        Err(e) => Err(e),
    }
}

/// Discard local changes to a path (GWEN-328). Untracked files are deleted from
/// disk; tracked files are restored from the index/HEAD.
pub fn discard(root: &Path, path: &str, untracked: bool) -> Result<(), GitError> {
    if untracked {
        // Remove the untracked file from the worktree (scoped to the one path).
        run_git(root, &["clean", "-fdq", "--", path]).map(|_| ())
    } else {
        // Drop both staged and unstaged changes for the path.
        let _ = run_git(root, &["reset", "-q", "HEAD", "--", path]);
        run_git(root, &["checkout", "--", path]).map(|_| ())
    }
}

// --- Commit / sync (GWEN-328) ----------------------------------------------

/// Commit the staged index with `message` (`git commit -m`).
pub fn commit(root: &Path, message: &str) -> Result<(), GitError> {
    run_git(root, &["commit", "-m", message]).map(|_| ())
}

/// Push the current branch (`git push`).
pub fn push(root: &Path) -> Result<String, GitError> {
    run_git(root, &["push"])
}

/// Pull with rebase off (`git pull`).
pub fn pull(root: &Path) -> Result<String, GitError> {
    run_git(root, &["pull"])
}

// --- Diff (GWEN-330) -------------------------------------------------------

/// Unified diff for one path. Includes staged changes (`HEAD`) so the viewer
/// shows the full delta whether or not the file is staged. Untracked files have
/// no diff base, so we synthesize one with `--no-index` against /dev/null.
pub fn diff_file(root: &Path, path: &str, untracked: bool) -> Result<String, GitError> {
    if untracked {
        // `git diff --no-index /dev/null <path>` exits 1 when they differ (which
        // they always do here), so treat a non-empty stdout as success.
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let args = ["diff", "--no-index", "--", null, path];
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(root);
        hide_child_window(&mut cmd);
        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                GitError::GitMissing
            } else {
                GitError::Io(e.to_string())
            }
        })?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        run_git(root, &["diff", "HEAD", "--", path])
    }
}

// --- Branches (GWEN-331) ---------------------------------------------------

/// All local branch names, current first stripped of the `* ` marker.
pub fn list_branches(root: &Path) -> Result<Vec<String>, GitError> {
    let out = run_git(root, &["branch", "--format=%(refname:short)"])?;
    Ok(out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Slugify a branch name: trim, lowercase nothing (git is case-sensitive) but
/// replace runs of whitespace with single hyphens (GWEN-331).
pub fn slugify_branch(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join("-")
}

/// Switch to an existing branch (`git checkout <branch>`).
pub fn checkout(root: &Path, branch: &str) -> Result<(), GitError> {
    run_git(root, &["checkout", branch]).map(|_| ())
}

/// Create and switch to a new branch (`git checkout -b <slug>`). The name is
/// slugified (spaces → hyphens) first.
pub fn create_branch(root: &Path, name: &str) -> Result<String, GitError> {
    let slug = slugify_branch(name);
    if slug.is_empty() {
        return Err(GitError::CommandFailed("branch name is empty".into()));
    }
    run_git(root, &["checkout", "-b", &slug])?;
    Ok(slug)
}

/// Delete a local branch (`git branch -D <branch>`). The caller must ensure it
/// is not the current branch (the command palette excludes it).
pub fn delete_branch(root: &Path, branch: &str) -> Result<(), GitError> {
    run_git(root, &["branch", "-D", branch]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_states() {
        let out = " M src/a.rs\nA  src/b.rs\n?? new.txt\nD  gone.rs\n";
        let files = parse_porcelain(out);
        assert_eq!(files.len(), 4);
        assert_eq!(files[0].path, "src/a.rs");
        assert_eq!(files[0].status, "M");
        assert!(!files[0].staged);
        assert_eq!(files[1].status, "A");
        assert!(files[1].staged);
        assert!(files[2].untracked);
        assert_eq!(files[2].status, "U");
        assert_eq!(files[3].status, "D");
        assert!(files[3].staged);
    }

    #[test]
    fn rename_keeps_new_path() {
        let files = parse_porcelain("R  old.rs -> new.rs\n");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "new.rs");
        assert_eq!(files[0].status, "R");
    }

    #[test]
    fn unquotes_paths() {
        assert_eq!(unquote_path("\"sp ace.rs\""), "sp ace.rs");
        assert_eq!(unquote_path("plain.rs"), "plain.rs");
    }

    #[test]
    fn slugifies_branch_names() {
        assert_eq!(slugify_branch("  my new feature "), "my-new-feature");
        assert_eq!(slugify_branch("fix/bug 12"), "fix/bug-12");
    }

    #[test]
    fn ahead_behind_parses_rev_list_output() {
        // Simulate what `git rev-list --count --left-right HEAD...@{u}` outputs.
        let parse = |s: &str| -> (usize, usize) {
            let mut parts = s.split_whitespace();
            let a = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            let b = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            (a, b)
        };
        assert_eq!(parse("3\t1"), (3, 1));
        assert_eq!(parse("0\t0"), (0, 0));
        assert_eq!(parse(""), (0, 0));
        assert_eq!(parse("5\t0"), (5, 0));
    }

    fn raw(hash: &str, parents: &[&str], refs: &[&str]) -> RawCommit {
        RawCommit {
            hash: hash.to_string(),
            short_hash: hash.chars().take(7).collect(),
            subject: format!("commit {hash}"),
            author: "Ada".to_string(),
            date: "2026-06-27T10:00:00+00:00".to_string(),
            parents: parents.iter().map(|parent| (*parent).to_string()).collect(),
            refs: refs.iter().map(|value| (*value).to_string()).collect(),
            branch_refs: Vec::new(),
        }
    }

    #[test]
    fn parses_git_log_line_with_full_refs() {
        let line = concat!(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|aaaaaaa|subject|Ada|",
            "2026-06-27T10:00:00+00:00|bbbbbbbb cccccccc|",
            "HEAD -> refs/heads/main, refs/remotes/origin/main, tag: refs/tags/v1"
        );

        let commit = parse_git_log_line(line).expect("line should parse");
        assert_eq!(commit.hash, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(commit.short_hash, "aaaaaaa");
        assert_eq!(commit.subject, "subject");
        assert_eq!(commit.parents, vec!["bbbbbbbb", "cccccccc"]);
        assert_eq!(commit.refs, vec!["HEAD", "main", "origin/main", "tag: v1"]);
        assert_eq!(
            commit.branch_refs,
            vec![
                ("main".to_string(), false),
                ("origin/main".to_string(), true)
            ]
        );
    }

    #[test]
    fn graph_lanes_keep_head_first_parent_on_top() {
        let commits = vec![
            raw("merge01", &["main002", "feat002"], &["HEAD", "main"]),
            raw("feat002", &["feat001"], &["feature/a"]),
            raw("feat001", &["main001"], &[]),
            raw("main002", &["main001"], &[]),
            raw("main001", &[], &[]),
        ];

        let nodes = assign_lanes(&commits);
        let lane = |hash: &str| {
            nodes
                .iter()
                .find(|node| node.hash == hash)
                .map(|node| node.lane)
                .unwrap()
        };

        assert_eq!(lane("merge01"), 0);
        assert_eq!(lane("main002"), 0);
        assert_eq!(lane("main001"), 0);
        assert_eq!(lane("feat002"), 1);
        assert_eq!(lane("feat001"), 1);
        assert!(nodes[0].is_head);
        assert!(nodes[0].is_merge);
    }

    #[test]
    fn graph_edges_follow_parent_relationships() {
        let commits = vec![
            raw("merge01", &["main002", "feat002"], &["HEAD", "main"]),
            raw("feat002", &["feat001"], &["feature/a"]),
            raw("feat001", &["main001"], &[]),
            raw("main002", &["main001"], &[]),
            raw("main001", &[], &[]),
        ];

        let nodes = assign_lanes(&commits);
        let edges = build_edges(&nodes);

        assert!(edges.iter().any(|edge| {
            edge.from == "merge01" && edge.to == "main002" && edge.kind == CommitEdgeKind::Linear
        }));
        assert!(edges.iter().any(|edge| {
            edge.from == "merge01" && edge.to == "feat002" && edge.kind == CommitEdgeKind::Fork
        }));
    }

    #[test]
    fn parses_commit_name_status() {
        let changes = parse_name_status(
            "M\tsrc/lib.rs\nA\tnew file.md\nD\told.rs\nR100\told name.rs\tnew name.rs\n",
        );

        assert_eq!(
            changes,
            vec![
                CommitFileChange {
                    path: "src/lib.rs".to_string(),
                    status: "M".to_string(),
                },
                CommitFileChange {
                    path: "new file.md".to_string(),
                    status: "A".to_string(),
                },
                CommitFileChange {
                    path: "old.rs".to_string(),
                    status: "D".to_string(),
                },
                CommitFileChange {
                    path: "new name.rs".to_string(),
                    status: "R".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parses_commit_numstat_totals() {
        assert_eq!(
            parse_numstat("12\t3\tsrc/lib.rs\n-\t-\tasset.bin\n4\t0\tnew.rs\n"),
            (16, 3)
        );
    }

    #[test]
    fn parses_commit_metadata_with_full_message() {
        let out = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\x1fAda\x1f2026-06-27T10:00:00+00:00\x1fSubject\n\nBody line\n";
        let (hash, author, date, message) = parse_commit_metadata(out).unwrap();
        assert_eq!(hash, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(author, "Ada");
        assert_eq!(date, "2026-06-27T10:00:00+00:00");
        assert_eq!(message, "Subject\n\nBody line");
    }

    #[test]
    fn rejects_non_hash_commit_ids() {
        assert!(validate_commit_hash("aaaaaaaa").is_ok());
        assert!(validate_commit_hash("--all").is_err());
        assert!(validate_commit_hash("main").is_err());
    }
}
