//! Local append-only audit log (M14 Wave 3).
//!
//! Audit events are written as compact JSONL under `.gwenland/audit/`.
//! One file per category so logs can be rotated/inspected independently.
//!
//! Design invariants:
//! - Each line is one JSON object (no arrays, no pretty-printing).
//! - Never write full file contents, API keys, provider auth headers, or
//!   full terminal history. Target summaries are bounded + redacted.
//! - Destructive actions fail closed if audit write fails.
//! - Read-only actions may continue with a warning on audit failure.
//! - No remote telemetry, no cloud dependencies.

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::agentic::policy::redact_secrets;
use crate::safety::decision::{RiskLevel, SafetyVerdict};

// ---------------------------------------------------------------------------
// Audit event schema
// ---------------------------------------------------------------------------

/// Log category → maps to one file under `.gwenland/audit/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditCategory {
    Safety,
    Agent,
    Terminal,
    Git,
    Extension,
    Rollback,
}

impl AuditCategory {
    pub fn filename(self) -> &'static str {
        match self {
            Self::Safety => "safety.jsonl",
            Self::Agent => "agent.jsonl",
            Self::Terminal => "terminal.jsonl",
            Self::Git => "git.jsonl",
            Self::Extension => "extension.jsonl",
            Self::Rollback => "rollback.jsonl",
        }
    }
}

/// High-level kind of the event (finer than `AuditCategory`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditKind {
    /// A safety decision was made (allow/ask/block).
    SafetyDecision,
    /// An agent proposed or executed an action.
    AgentAction,
    /// A terminal command was approved and executed.
    TerminalCommand,
    /// A git operation completed or was blocked.
    GitOperation,
    /// An extension was granted or denied a permission.
    ExtensionPermission,
    /// A file/snapshot/trash/backup rollback was performed.
    RollbackAction,
    /// A destructive action was blocked due to audit write failure.
    AuditFailureBlock,
}

/// One audit event — safe to serialize and store. Must never contain secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    /// RFC-3339 UTC timestamp.
    pub timestamp: String,
    /// Absolute workspace root path.
    pub workspace_root: String,
    /// Who initiated the action.
    pub actor: String,
    pub category: AuditCategory,
    pub kind: AuditKind,
    pub risk: RiskLevel,
    pub verdict: SafetyVerdict,
    /// Human-readable reason for the verdict (no secrets).
    pub reason: String,
    /// Bounded, redacted summary of the target (path, command, etc.).
    pub target_summary: String,
    /// Optional caller correlation id (e.g. agent session id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl AuditEvent {
    /// Maximum characters for `target_summary` (prevents large log entries).
    const MAX_SUMMARY: usize = 256;

    /// Build an event from a `SafetyDecision`. The `target_summary` is
    /// redacted and bounded before storage.
    pub fn from_decision(
        decision: &crate::safety::decision::SafetyDecision,
        action: &crate::safety::action::SafetyAction,
        category: AuditCategory,
        kind: AuditKind,
    ) -> Self {
        let raw_summary = match &action.kind {
            crate::safety::action::SafetyActionKind::FileRead { path }
            | crate::safety::action::SafetyActionKind::FileCreate { path }
            | crate::safety::action::SafetyActionKind::FileWrite { path }
            | crate::safety::action::SafetyActionKind::FileDelete { path } => path.clone(),
            crate::safety::action::SafetyActionKind::FileRename { old_path, new_path } => {
                format!("{old_path} → {new_path}")
            }
            crate::safety::action::SafetyActionKind::FileCopy { src, dest } => {
                format!("{src} → {dest}")
            }
            crate::safety::action::SafetyActionKind::TerminalCommand { command } => {
                command.clone()
            }
            crate::safety::action::SafetyActionKind::GitCommit { message_summary } => {
                message_summary.clone()
            }
            crate::safety::action::SafetyActionKind::GitCheckout { target } => target.clone(),
            crate::safety::action::SafetyActionKind::GitBranchDelete { branch } => {
                branch.clone()
            }
            crate::safety::action::SafetyActionKind::GitDestructive { summary }
            | crate::safety::action::SafetyActionKind::GitRemote { summary }
            | crate::safety::action::SafetyActionKind::Unknown { summary } => summary.clone(),
            crate::safety::action::SafetyActionKind::AiContextInclude { path_count, .. } => {
                format!("{path_count} file(s)")
            }
            crate::safety::action::SafetyActionKind::AiResponseStore => "ai response".into(),
            crate::safety::action::SafetyActionKind::GitRead => "git read".into(),
            crate::safety::action::SafetyActionKind::ExtensionPermission {
                extension_id,
                permission,
            } => format!("{extension_id}:{permission}"),
            crate::safety::action::SafetyActionKind::RemoteExport { destination_summary } => {
                destination_summary.clone()
            }
        };

        let (redacted, _) = redact_secrets(&raw_summary);
        let target_summary = bound_str(&redacted, Self::MAX_SUMMARY);

        Self {
            id: action.id.clone(),
            timestamp: crate::agentic::now_rfc3339(),
            workspace_root: action.workspace_root.clone(),
            actor: action.actor.to_string(),
            category,
            kind,
            risk: decision.risk,
            verdict: decision.verdict.clone(),
            reason: bound_str(&decision.reason, 512),
            target_summary,
            correlation_id: action.correlation_id.clone(),
        }
    }

    /// Build a simple audit event (for rollback, agent, terminal events that
    /// don't have a `SafetyDecision`).
    pub fn simple(
        id: impl Into<String>,
        workspace_root: impl Into<String>,
        actor: impl Into<String>,
        category: AuditCategory,
        kind: AuditKind,
        risk: RiskLevel,
        verdict: SafetyVerdict,
        reason: impl Into<String>,
        target_summary: impl Into<String>,
    ) -> Self {
        let raw = target_summary.into();
        let (redacted, _) = redact_secrets(&raw);
        Self {
            id: id.into(),
            timestamp: crate::agentic::now_rfc3339(),
            workspace_root: workspace_root.into(),
            actor: actor.into(),
            category,
            kind,
            risk,
            verdict,
            reason: reason.into(),
            target_summary: bound_str(&redacted, Self::MAX_SUMMARY),
            correlation_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Audit writer
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("failed to create audit directory: {0}")]
    CreateDir(std::io::Error),
    #[error("failed to open audit log: {0}")]
    OpenFile(std::io::Error),
    #[error("failed to serialize audit event: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to write audit log: {0}")]
    Write(std::io::Error),
}

/// Append-only audit log writer. Each instance is stateless (no file handle
/// held between writes) so it is `Send + Sync` without a `Mutex`.
pub struct AuditWriter {
    audit_dir: PathBuf,
}

impl AuditWriter {
    /// Create a writer targeting `.gwenland/audit/` under `workspace_root`.
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            audit_dir: crate::workspace::audit_dir(workspace_root),
        }
    }

    /// Append `event` to the appropriate category JSONL file.
    ///
    /// The audit directory is created lazily so no `.gwenland/` directory
    /// needs to exist at `AuditWriter` construction time.
    pub fn append(&self, event: &AuditEvent) -> Result<(), AuditError> {
        std::fs::create_dir_all(&self.audit_dir).map_err(AuditError::CreateDir)?;
        let path = self.audit_dir.join(event.category.filename());
        let mut line = serde_json::to_string(event).map_err(AuditError::Serialize)?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(AuditError::OpenFile)?;
        file.write_all(line.as_bytes()).map_err(AuditError::Write)?;
        // Flush: on most platforms `write_all` on a file is already flushed to
        // the kernel buffer; an explicit flush ensures libc stdio is flushed.
        let _ = file.flush();
        Ok(())
    }

    /// Append a `SafetyDecision` as an audit event (convenience wrapper).
    pub fn record_decision(
        &self,
        decision: &crate::safety::decision::SafetyDecision,
        action: &crate::safety::action::SafetyAction,
        category: AuditCategory,
        kind: AuditKind,
    ) -> Result<(), AuditError> {
        let event = AuditEvent::from_decision(decision, action, category, kind);
        self.append(&event)
    }

    /// Read all lines from `category` log, skipping malformed lines.
    /// Used only for tests; in production, logs are consumed by log viewers.
    pub fn read_all(&self, category: AuditCategory) -> Vec<AuditEvent> {
        let path = self.audit_dir.join(category.filename());
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        content
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
}

/// Should a destructive action be blocked if its audit write fails?
/// Returns `true` for destructive/secret actions, `false` for safe/read-only.
pub fn should_block_on_audit_failure(risk: RiskLevel) -> bool {
    matches!(
        risk,
        RiskLevel::Destructive | RiskLevel::Secret | RiskLevel::High | RiskLevel::Unknown
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bound_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::action::{Actor, SafetyAction, SafetyActionKind};
    use crate::safety::decision::{ConfirmationKind, SafetyDecision};
    use tempfile::tempdir;

    fn sample_decision(id: &str) -> (SafetyDecision, SafetyAction) {
        let action = SafetyAction::new(
            Actor::Agent,
            SafetyActionKind::FileDelete { path: "src/old.rs".into() },
            "/workspace",
        );
        let decision = SafetyDecision::ask(
            id,
            RiskLevel::High,
            "file delete requires confirmation",
            ConfirmationKind::Simple,
        );
        (decision, action)
    }

    // 3.6.1 — audit file is created under workspace
    #[test]
    fn audit_file_created_under_workspace() {
        let dir = tempdir().unwrap();
        let writer = AuditWriter::new(dir.path());
        let (decision, action) = sample_decision("id-1");
        writer
            .record_decision(&decision, &action, AuditCategory::Safety, AuditKind::SafetyDecision)
            .unwrap();
        let log_path = dir.path().join(".gwenland/audit/safety.jsonl");
        assert!(log_path.exists(), "audit log must exist");
    }

    // 3.6.2 — events append without overwriting
    #[test]
    fn events_append_without_overwriting() {
        let dir = tempdir().unwrap();
        let writer = AuditWriter::new(dir.path());
        for i in 0..3 {
            let (decision, action) = sample_decision(&format!("id-{i}"));
            writer
                .record_decision(
                    &decision,
                    &action,
                    AuditCategory::Safety,
                    AuditKind::SafetyDecision,
                )
                .unwrap();
        }
        let events = writer.read_all(AuditCategory::Safety);
        assert_eq!(events.len(), 3, "all 3 events must be present");
    }

    // 3.6.3 — malformed existing lines do not prevent future appends
    #[test]
    fn malformed_lines_do_not_block_append() {
        let dir = tempdir().unwrap();
        // Pre-populate with a corrupt line.
        std::fs::create_dir_all(dir.path().join(".gwenland/audit")).unwrap();
        std::fs::write(
            dir.path().join(".gwenland/audit/safety.jsonl"),
            b"not-json\n",
        )
        .unwrap();
        let writer = AuditWriter::new(dir.path());
        let (decision, action) = sample_decision("ignored");
        let action_id = action.id.clone();
        writer
            .record_decision(&decision, &action, AuditCategory::Safety, AuditKind::SafetyDecision)
            .unwrap();
        // read_all skips the corrupt line; only the valid one appears.
        let events = writer.read_all(AuditCategory::Safety);
        assert_eq!(events.len(), 1);
        // The audit event id comes from the action id (a random UUID).
        assert_eq!(events[0].id, action_id);
    }

    // 3.6.4 — redaction runs before write
    #[test]
    fn redaction_runs_before_write() {
        let dir = tempdir().unwrap();
        let writer = AuditWriter::new(dir.path());
        let action = SafetyAction::new(
            Actor::Agent,
            SafetyActionKind::TerminalCommand {
                command: "export KEY=sk-ant-abcdefghijklmnopqrstuvwxyz01234".into(),
            },
            "/workspace",
        );
        let decision = SafetyDecision::ask(
            "id-redact",
            RiskLevel::Medium,
            "terminal command",
            ConfirmationKind::Simple,
        );
        writer
            .record_decision(
                &decision,
                &action,
                AuditCategory::Terminal,
                AuditKind::TerminalCommand,
            )
            .unwrap();

        let log_path = dir.path().join(".gwenland/audit/terminal.jsonl");
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            !content.contains("sk-ant-abcdefghijklmnopq"),
            "secret token must not appear in audit log"
        );
        assert!(
            content.contains("[REDACTED]"),
            "redaction placeholder must appear"
        );
    }

    // 3.6.5 — should_block_on_audit_failure is conservative for risky levels
    #[test]
    fn block_on_audit_failure_is_conservative() {
        assert!(should_block_on_audit_failure(RiskLevel::Destructive));
        assert!(should_block_on_audit_failure(RiskLevel::Secret));
        assert!(should_block_on_audit_failure(RiskLevel::High));
        assert!(should_block_on_audit_failure(RiskLevel::Unknown));
        assert!(!should_block_on_audit_failure(RiskLevel::Safe));
        assert!(!should_block_on_audit_failure(RiskLevel::Low));
    }

    // 3.6.6 — target summary is bounded
    #[test]
    fn target_summary_is_bounded() {
        let dir = tempdir().unwrap();
        let writer = AuditWriter::new(dir.path());
        let long_path = "a".repeat(512);
        let action = SafetyAction::new(
            Actor::User,
            SafetyActionKind::FileDelete { path: long_path },
            "/workspace",
        );
        let decision = SafetyDecision::ask(
            "id-long",
            RiskLevel::High,
            "very long path",
            ConfirmationKind::Simple,
        );
        writer
            .record_decision(&decision, &action, AuditCategory::Safety, AuditKind::SafetyDecision)
            .unwrap();
        let events = writer.read_all(AuditCategory::Safety);
        assert!(
            events[0].target_summary.len() <= AuditEvent::MAX_SUMMARY + 3,
            "target_summary must be bounded"
        );
    }

    // 3.6.7 — each category uses a separate file
    #[test]
    fn each_category_uses_separate_file() {
        let dir = tempdir().unwrap();
        let writer = AuditWriter::new(dir.path());
        let event = AuditEvent::simple(
            "id-git",
            "/workspace",
            "user",
            AuditCategory::Git,
            AuditKind::GitOperation,
            RiskLevel::Medium,
            SafetyVerdict::Allow,
            "git commit",
            "Initial commit",
        );
        writer.append(&event).unwrap();
        assert!(dir.path().join(".gwenland/audit/git.jsonl").exists());
        assert!(!dir.path().join(".gwenland/audit/safety.jsonl").exists());
    }
}
