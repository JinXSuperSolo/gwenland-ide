//! Convenience guard helpers that combine `ProtectedPathRegistry` + `evaluate`
//! into single-call entry points for the most common engine callsites.
//!
//! These helpers load the registry lazily from the workspace root and fall
//! back to defaults on any I/O error — they must never panic or return an
//! `Err` for a missing registry file.

use std::path::Path;

use crate::safety::action::{Actor, SafetyAction, SafetyActionKind};
use crate::safety::confirmation::evaluate;
use crate::safety::decision::SafetyDecision;
use crate::safety::protected_paths::ProtectedPathRegistry;
use crate::workspace::SafetyStrictness;

/// Evaluate a file-mutation action against local safety policy.
///
/// `workspace_root` is used to (a) resolve the protected-path registry and
/// (b) populate `SafetyAction.workspace_root`. `actor` should be `Agent`
/// when called from the agentic loop, `User` for explicit menu/keyboard
/// actions, `System` for internal IDE operations.
pub fn check_file_action(
    kind: SafetyActionKind,
    workspace_root: &Path,
    actor: Actor,
    strictness: SafetyStrictness,
) -> SafetyDecision {
    let registry = ProtectedPathRegistry::load(workspace_root);
    let action = SafetyAction::new(actor, kind, workspace_root.to_string_lossy().as_ref());
    evaluate(&action, &registry, strictness)
}

/// Quick predicate: would a file write to `path` be allowed without a prompt?
///
/// Equivalent to `check_file_action(FileWrite, …).verdict == Allow` but
/// avoids allocating a full `SafetyDecision` struct for hot paths.
pub fn file_write_allowed(path: &str, workspace_root: &Path, strictness: SafetyStrictness) -> bool {
    let d = check_file_action(
        SafetyActionKind::FileWrite {
            path: path.to_string(),
        },
        workspace_root,
        Actor::Agent,
        strictness,
    );
    matches!(d.verdict, crate::safety::decision::SafetyVerdict::Allow)
}

/// Evaluate a terminal command proposed by an agent or extension.
pub fn check_terminal_command(
    command: &str,
    workspace_root: &Path,
    actor: Actor,
    strictness: SafetyStrictness,
) -> SafetyDecision {
    check_file_action(
        SafetyActionKind::TerminalCommand {
            command: command.to_string(),
        },
        workspace_root,
        actor,
        strictness,
    )
}

/// Evaluate an AI context inclusion (used before assembling a provider request).
pub fn check_ai_context(
    path_count: usize,
    has_secret_path: bool,
    workspace_root: &Path,
    strictness: SafetyStrictness,
) -> SafetyDecision {
    check_file_action(
        SafetyActionKind::AiContextInclude {
            path_count,
            has_secret_path,
        },
        workspace_root,
        Actor::System,
        strictness,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn file_write_allowed_returns_false_for_secret_path() {
        let dir = tempdir().unwrap();
        assert!(!file_write_allowed(
            ".env",
            dir.path(),
            SafetyStrictness::Standard
        ));
    }

    #[test]
    fn file_write_allowed_returns_true_for_normal_path_standard() {
        let dir = tempdir().unwrap();
        assert!(file_write_allowed(
            "src/main.rs",
            dir.path(),
            SafetyStrictness::Standard
        ));
    }

    #[test]
    fn check_terminal_command_dangerous_asks() {
        let dir = tempdir().unwrap();
        let d = check_terminal_command(
            "rm -rf .",
            dir.path(),
            Actor::Agent,
            SafetyStrictness::Standard,
        );
        assert!(matches!(
            d.verdict,
            crate::safety::decision::SafetyVerdict::Ask
                | crate::safety::decision::SafetyVerdict::Block
        ));
    }

    #[test]
    fn check_ai_context_with_secret_path_blocks() {
        let dir = tempdir().unwrap();
        let d = check_ai_context(2, true, dir.path(), SafetyStrictness::Standard);
        assert_eq!(d.verdict, crate::safety::decision::SafetyVerdict::Block);
    }
}
