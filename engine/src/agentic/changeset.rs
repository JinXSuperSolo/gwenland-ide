//! Reviewable change sets (M10, Requirements 5 & 6).
//!
//! A [`ChangeSet`] is the structured, approval-bearing form of the assistant's
//! proposed edits. It is built by wrapping the existing unified-diff parser
//! ([`crate::ai::diff`]) so M10 reuses one battle-tested parser rather than
//! adding another. Raw assistant prose is never applied — only parsed hunks the
//! user approves (enforced Tauri-side in Wave 5).
//!
//! No file writes happen here; this module is pure data + parsing.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ai::diff::{self, DiffLine};

/// Per-file/per-hunk approval state (Requirement 5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ApprovalState {
    #[default]
    Pending,
    Approved,
    Rejected,
    /// Application was attempted and failed (e.g. hunk conflict).
    Failed,
}

/// The kind of file-level change a proposed file represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Modify,
    Create,
    Delete,
    Rename,
}

impl FileChangeKind {
    /// Delete / rename are inherently destructive and need explicit confirmation
    /// before apply (Requirement 6.3).
    pub fn is_destructive(self) -> bool {
        matches!(self, FileChangeKind::Delete | FileChangeKind::Rename)
    }
}

/// One reviewable hunk. Mirrors [`diff::DiffHunk`] plus an id and approval state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedHunk {
    pub id: String,
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub header: String,
    pub lines: Vec<DiffLine>,
    pub approval: ApprovalState,
}

/// One file's proposed change with its hunks and roll-up approval state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedFileChange {
    pub id: String,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub change_kind: FileChangeKind,
    pub hunks: Vec<ProposedHunk>,
    pub approval: ApprovalState,
}

impl ProposedFileChange {
    /// The path edits should target (new path, falling back to old).
    pub fn target_path(&self) -> Option<&str> {
        self.new_path.as_deref().or(self.old_path.as_deref())
    }

    /// True if any hunk in this file is approved.
    pub fn has_approved_hunk(&self) -> bool {
        self.approval == ApprovalState::Approved
            || self
                .hunks
                .iter()
                .any(|h| h.approval == ApprovalState::Approved)
    }
}

/// A structured, reviewable set of proposed file changes for one plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSet {
    pub id: String,
    pub plan_id: String,
    pub files: Vec<ProposedFileChange>,
    /// Non-fatal parse notices (e.g. a file with no parseable hunks). Their
    /// presence makes the set review-blocking until resolved (Requirement 5.7).
    pub parse_warnings: Vec<String>,
}

impl ChangeSet {
    /// Whether at least one hunk or whole file is approved and thus eligible for
    /// application (Requirement 6.1).
    pub fn has_approved_change(&self) -> bool {
        self.files.iter().any(|f| f.has_approved_hunk())
    }

    /// Find a hunk by id and set its approval state. Returns true if found.
    pub fn set_hunk_approval(&mut self, hunk_id: &str, state: ApprovalState) -> bool {
        for file in &mut self.files {
            if let Some(h) = file.hunks.iter_mut().find(|h| h.id == hunk_id) {
                h.approval = state;
                return true;
            }
        }
        false
    }

    /// Set every hunk of `file_id` (and the file roll-up) to `state`.
    pub fn set_file_approval(&mut self, file_id: &str, state: ApprovalState) -> bool {
        if let Some(file) = self.files.iter_mut().find(|f| f.id == file_id) {
            file.approval = state;
            for h in &mut file.hunks {
                h.approval = state;
            }
            true
        } else {
            false
        }
    }

    /// Whether the set is empty of any usable change (no files / no hunks).
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() || self.files.iter().all(|f| f.hunks.is_empty())
    }
}

/// Apply only approved hunks from one file change to `content`, validating the
/// old/context lines before changing anything. Rejected/pending hunks are
/// ignored. This is pure text logic; callers own workspace checks and file I/O.
pub fn apply_approved_hunks_to_text(
    content: &str,
    file: &ProposedFileChange,
) -> Result<String, String> {
    let mut hunks: Vec<&ProposedHunk> = file
        .hunks
        .iter()
        .filter(|h| h.approval == ApprovalState::Approved)
        .collect();
    if hunks.is_empty() {
        return Ok(content.to_string());
    }

    // Apply bottom-to-top so earlier edits do not shift later hunk offsets.
    hunks.sort_by_key(|b| std::cmp::Reverse(b.old_start));

    let (mut lines, line_ending, trailing_newline) = split_text_lines(content);
    for hunk in hunks {
        apply_one_hunk(&mut lines, hunk)?;
    }
    Ok(join_text_lines(&lines, line_ending, trailing_newline))
}

fn split_text_lines(content: &str) -> (Vec<String>, &'static str, bool) {
    let line_ending = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let trailing_newline = content.ends_with(line_ending);
    let body = if trailing_newline {
        &content[..content.len().saturating_sub(line_ending.len())]
    } else {
        content
    };
    let lines = if body.is_empty() {
        Vec::new()
    } else {
        body.split(line_ending).map(|s| s.to_string()).collect()
    };
    (lines, line_ending, trailing_newline)
}

fn join_text_lines(lines: &[String], line_ending: &str, trailing_newline: bool) -> String {
    let mut out = lines.join(line_ending);
    if trailing_newline && !out.is_empty() {
        out.push_str(line_ending);
    }
    out
}

fn apply_one_hunk(lines: &mut Vec<String>, hunk: &ProposedHunk) -> Result<(), String> {
    let mut expected_old = Vec::new();
    let mut replacement = Vec::new();
    for line in &hunk.lines {
        match line {
            DiffLine::Context(text) => {
                expected_old.push(text.clone());
                replacement.push(text.clone());
            }
            DiffLine::Removed(text) => expected_old.push(text.clone()),
            DiffLine::Added(text) => replacement.push(text.clone()),
        }
    }

    let start = if hunk.old_start == 0 {
        0
    } else {
        hunk.old_start - 1
    };
    let end = start.saturating_add(expected_old.len());
    if end > lines.len() {
        return Err(format!(
            "hunk at -{},{} is outside the current file",
            hunk.old_start, hunk.old_count
        ));
    }
    if lines[start..end] != expected_old[..] {
        return Err(format!(
            "hunk context mismatch at line {}",
            if hunk.old_start == 0 {
                1
            } else {
                hunk.old_start
            }
        ));
    }
    lines.splice(start..end, replacement);
    Ok(())
}

/// Derive the file-level change kind from old/new paths.
fn change_kind(old: &Option<String>, new: &Option<String>) -> FileChangeKind {
    match (old, new) {
        (None, Some(_)) => FileChangeKind::Create,
        (Some(_), None) => FileChangeKind::Delete,
        (Some(o), Some(n)) if o != n => FileChangeKind::Rename,
        _ => FileChangeKind::Modify,
    }
}

/// Convert already-parsed [`diff::DiffFile`]s into a [`ChangeSet`], assigning
/// ids to files and hunks. `warnings` is carried through verbatim.
pub fn change_set_from_diff_files(
    plan_id: impl Into<String>,
    files: Vec<diff::DiffFile>,
    warnings: Vec<String>,
) -> ChangeSet {
    let proposed = files
        .into_iter()
        .map(|f| {
            let change_kind = change_kind(&f.old_path, &f.new_path);
            let hunks = f
                .hunks
                .into_iter()
                .map(|h| ProposedHunk {
                    id: Uuid::new_v4().to_string(),
                    old_start: h.old_start,
                    old_count: h.old_count,
                    new_start: h.new_start,
                    new_count: h.new_count,
                    header: h.header,
                    lines: h.lines,
                    approval: ApprovalState::Pending,
                })
                .collect();
            ProposedFileChange {
                id: Uuid::new_v4().to_string(),
                old_path: f.old_path,
                new_path: f.new_path,
                change_kind,
                hunks,
                approval: ApprovalState::Pending,
            }
        })
        .collect();

    ChangeSet {
        id: Uuid::new_v4().to_string(),
        plan_id: plan_id.into(),
        files: proposed,
        parse_warnings: warnings,
    }
}

/// Parse assistant text into a [`ChangeSet`] (Requirement 5.2/5.4/5.7). On a
/// malformed-diff parse error the set has no files and a parse warning, so the
/// UI keeps the assistant text visible and offers no apply button. Files that
/// parsed with zero hunks also produce a warning.
pub fn change_set_from_text(plan_id: impl Into<String>, text: &str) -> ChangeSet {
    let plan_id = plan_id.into();
    match diff::parse_unified_diff(text) {
        Ok(files) => {
            let mut warnings = Vec::new();
            let usable: Vec<diff::DiffFile> =
                files.into_iter().filter(|f| !f.hunks.is_empty()).collect();
            if usable.is_empty() {
                warnings.push(
                    "No applyable changes were found in the response. Ask for a revision with unified diffs.".to_string(),
                );
            }
            change_set_from_diff_files(plan_id, usable, warnings)
        }
        Err(e) => ChangeSet {
            id: Uuid::new_v4().to_string(),
            plan_id,
            files: Vec::new(),
            parse_warnings: vec![format!("Could not parse the proposed diff: {e}")],
        },
    }
}

// --- Apply report ----------------------------------------------------------

/// One file's apply outcome (used in each bucket of [`ApplyReport`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyOutcome {
    pub file_id: String,
    pub path: String,
    pub hunk_ids: Vec<String>,
    /// User-safe explanation (e.g. "applied 2 hunks", "hunk conflict at line 14").
    pub message: String,
}

/// Result of an apply pass, with applied/rejected/skipped/failed separated
/// (Requirement 6.7).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyReport {
    pub applied: Vec<ApplyOutcome>,
    pub rejected: Vec<ApplyOutcome>,
    pub skipped: Vec<ApplyOutcome>,
    pub failed: Vec<ApplyOutcome>,
}

impl ApplyReport {
    pub fn is_empty(&self) -> bool {
        self.applied.is_empty()
            && self.rejected.is_empty()
            && self.skipped.is_empty()
            && self.failed.is_empty()
    }

    /// Total files touched in any bucket.
    pub fn total(&self) -> usize {
        self.applied.len() + self.rejected.len() + self.skipped.len() + self.failed.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_diff_becomes_change_set() {
        let text = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
 fn main() {
-    println!(\"old\");
+    println!(\"new\");
 }
";
        let cs = change_set_from_text("plan-1", text);
        assert_eq!(cs.plan_id, "plan-1");
        assert_eq!(cs.files.len(), 1);
        assert!(cs.parse_warnings.is_empty());
        let f = &cs.files[0];
        assert_eq!(f.change_kind, FileChangeKind::Modify);
        assert_eq!(f.hunks.len(), 1);
        assert_eq!(f.hunks[0].approval, ApprovalState::Pending);
        assert!(!f.hunks[0].id.is_empty());
    }

    #[test]
    fn create_and_delete_kinds_detected() {
        let create = "--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,1 @@\n+hi\n";
        let cs = change_set_from_text("p", create);
        assert_eq!(cs.files[0].change_kind, FileChangeKind::Create);

        let delete = "--- a/gone.txt\n+++ /dev/null\n@@ -1,1 +0,0 @@\n-bye\n";
        let cs2 = change_set_from_text("p", delete);
        assert_eq!(cs2.files[0].change_kind, FileChangeKind::Delete);
        assert!(cs2.files[0].change_kind.is_destructive());
    }

    #[test]
    fn prose_without_diff_yields_warning_and_no_files() {
        let cs = change_set_from_text("p", "Sure, I'll update the file for you!");
        assert!(cs.files.is_empty());
        assert_eq!(cs.parse_warnings.len(), 1);
        assert!(!cs.has_approved_change());
    }

    #[test]
    fn approval_toggling_and_eligibility() {
        let text = "--- a/f\n+++ b/f\n@@ -1,1 +1,1 @@\n-a\n+b\n";
        let mut cs = change_set_from_text("p", text);
        let hunk_id = cs.files[0].hunks[0].id.clone();
        assert!(!cs.has_approved_change());
        assert!(cs.set_hunk_approval(&hunk_id, ApprovalState::Approved));
        assert!(cs.has_approved_change());
        assert!(cs.set_hunk_approval(&hunk_id, ApprovalState::Rejected));
        assert!(!cs.has_approved_change());
        assert!(!cs.set_hunk_approval("missing", ApprovalState::Approved));
    }

    #[test]
    fn approved_hunk_applies_to_text() {
        let text = "--- a/f\n+++ b/f\n@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n";
        let mut cs = change_set_from_text("p", text);
        let hunk_id = cs.files[0].hunks[0].id.clone();
        cs.set_hunk_approval(&hunk_id, ApprovalState::Approved);
        let out = apply_approved_hunks_to_text("a\nb\nc\n", &cs.files[0]).unwrap();
        assert_eq!(out, "a\nB\nc\n");
    }

    #[test]
    fn rejected_hunk_is_not_applied() {
        let text = "--- a/f\n+++ b/f\n@@ -1,1 +1,1 @@\n-a\n+b\n";
        let mut cs = change_set_from_text("p", text);
        let hunk_id = cs.files[0].hunks[0].id.clone();
        cs.set_hunk_approval(&hunk_id, ApprovalState::Rejected);
        let out = apply_approved_hunks_to_text("a", &cs.files[0]).unwrap();
        assert_eq!(out, "a");
    }

    #[test]
    fn conflicted_hunk_fails_without_output() {
        let text = "--- a/f\n+++ b/f\n@@ -1,1 +1,1 @@\n-a\n+b\n";
        let mut cs = change_set_from_text("p", text);
        let hunk_id = cs.files[0].hunks[0].id.clone();
        cs.set_hunk_approval(&hunk_id, ApprovalState::Approved);
        let err = apply_approved_hunks_to_text("not-a", &cs.files[0]).unwrap_err();
        assert!(err.contains("context mismatch"));
    }
}
