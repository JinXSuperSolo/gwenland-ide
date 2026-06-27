//! Git integration (Milestone "Wave 2" — GWEN-327..331).
//!
//! A thin, dependency-free wrapper over the system `git` binary (`std::process`
//! only — no git2/libgit2, to respect the binary budget). Every operation is
//! scoped to a workspace root passed by the caller; nothing here touches global
//! git config. All functions return a normalized [`GitError`] on failure.
//!
//! The frontend never shells out itself — it calls the `git_*` Tauri commands,
//! which delegate here.

use std::path::Path;
use std::process::Command;

use serde::Serialize;
use thiserror::Error;

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
}
