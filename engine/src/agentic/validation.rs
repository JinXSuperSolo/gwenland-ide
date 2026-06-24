//! Validation command model and result DTOs (M10, Requirement 7).
//!
//! A validation command (e.g. `cargo check`, `pnpm build`) is a *proposal* until
//! the user approves it — nothing here runs anything. This module defines the
//! command shape, its risk classification, and the captured-result DTOs. The
//! actual classification logic lives in [`crate::agentic::policy`]; the run/exec
//! plumbing lives Tauri-side in `frontend/src/`.
//!
//! Like the rest of `engine/src/agentic`, this module has ZERO Tauri/UI imports.

use serde::{Deserialize, Serialize};

/// How dangerous a proposed command is. The classifier is deliberately
/// conservative: anything it cannot confidently place lands in [`Blocked`].
///
/// Serializes snake_case (`safe_check`, `dependency_changing`, …).
///
/// [`Blocked`]: CommandRisk::Blocked
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandRisk {
    /// Build/test/typecheck/lint/format-check — read-or-build only.
    SafeCheck,
    /// Installs/adds/updates dependencies; requires a size-impact note (Req 7.5).
    DependencyChanging,
    /// Writes files in place (format-write, codegen, migrations).
    FileMutating,
    /// Deletes/resets/force-pushes/overwrites; requires danger confirmation (Req 7.6).
    Destructive,
    /// Cannot be safely classified, or targets outside the workspace. Blocked
    /// from running until reclassified (Req 7.6).
    Blocked,
}

impl CommandRisk {
    /// Stable, user-facing label.
    pub fn label(self) -> &'static str {
        match self {
            CommandRisk::SafeCheck => "safe check",
            CommandRisk::DependencyChanging => "dependency changing",
            CommandRisk::FileMutating => "file mutating",
            CommandRisk::Destructive => "destructive",
            CommandRisk::Blocked => "blocked",
        }
    }

    /// Dependency-changing commands must carry a size-impact note (Req 7.5).
    pub fn requires_size_impact_note(self) -> bool {
        matches!(self, CommandRisk::DependencyChanging)
    }

    /// Destructive commands require an explicit danger confirmation (Req 7.6).
    pub fn requires_danger_confirmation(self) -> bool {
        matches!(self, CommandRisk::Destructive)
    }

    /// `Blocked` commands can never be approved to run as-is.
    pub fn is_blocked(self) -> bool {
        matches!(self, CommandRisk::Blocked)
    }
}

/// A proposed validation command. `id` correlates the proposal to its approval
/// and any [`ValidationRun`]. Carries no environment or secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationCommand {
    pub id: String,
    /// The full command line as it would be run, e.g. `cargo check --workspace`.
    pub command: String,
    /// Working directory (must be inside the workspace; enforced before run).
    pub cwd: String,
    /// Why the agent suggests this command.
    pub reason: String,
    /// Conservative risk classification.
    pub risk: CommandRisk,
    /// Required for `DependencyChanging` before approval (Req 7.5). `None` until
    /// the user supplies it.
    pub size_impact_note: Option<String>,
}

impl ValidationCommand {
    /// Build a command, classifying its risk via [`crate::agentic::policy`].
    pub fn new(
        id: impl Into<String>,
        command: impl Into<String>,
        cwd: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let command = command.into();
        let risk = crate::agentic::policy::classify_command(&command);
        Self {
            id: id.into(),
            command,
            cwd: cwd.into(),
            reason: reason.into(),
            risk,
            size_impact_note: None,
        }
    }

    /// Whether this command is eligible for an approval request right now:
    /// not blocked, and any required size-impact note is present.
    pub fn is_approvable(&self) -> bool {
        if self.risk.is_blocked() {
            return false;
        }
        if self.risk.requires_size_impact_note()
            && self
                .size_impact_note
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            return false;
        }
        true
    }
}

/// Outcome status of a validation run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Pending,
    Running,
    Passed,
    Failed,
    /// Refused by policy (e.g. risk re-check failed at run time).
    Blocked,
    Cancelled,
}

/// A captured validation run. `output_excerpt` is bounded and may be redacted
/// (Req 7.7); the full output is never persisted verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationRun {
    pub id: String,
    pub command_id: String,
    pub status: ValidationStatus,
    pub exit_code: Option<i32>,
    pub output_excerpt: String,
    pub started_at: String,
    pub finished_at: Option<String>,
}

impl ValidationRun {
    /// Start a run in `Running` state.
    pub fn started(
        id: impl Into<String>,
        command_id: impl Into<String>,
        started_at: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            command_id: command_id.into(),
            status: ValidationStatus::Running,
            exit_code: None,
            output_excerpt: String::new(),
            started_at: started_at.into(),
            finished_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_serializes_snake_case() {
        let v = serde_json::to_value(CommandRisk::DependencyChanging).unwrap();
        assert_eq!(v, serde_json::json!("dependency_changing"));
    }

    #[test]
    fn dependency_command_needs_size_note_to_be_approvable() {
        let mut cmd = ValidationCommand::new("c1", "pnpm install", ".", "install deps");
        assert_eq!(cmd.risk, CommandRisk::DependencyChanging);
        assert!(
            !cmd.is_approvable(),
            "missing size-impact note must block approval"
        );
        cmd.size_impact_note = Some("adds ~1.2MB".into());
        assert!(cmd.is_approvable());
    }

    #[test]
    fn blocked_command_is_never_approvable() {
        let cmd = ValidationCommand::new("c2", "frobnicate --wat", ".", "???");
        assert_eq!(cmd.risk, CommandRisk::Blocked);
        assert!(!cmd.is_approvable());
    }

    #[test]
    fn safe_check_is_approvable_without_note() {
        let cmd = ValidationCommand::new("c3", "cargo check --workspace", ".", "verify build");
        assert_eq!(cmd.risk, CommandRisk::SafeCheck);
        assert!(cmd.is_approvable());
    }

    #[test]
    fn destructive_risk_requires_danger_confirmation() {
        let cmd = ValidationCommand::new("c4", "git reset --hard", ".", "dangerous check");
        assert_eq!(cmd.risk, CommandRisk::Destructive);
        assert!(cmd.risk.requires_danger_confirmation());
    }
}
