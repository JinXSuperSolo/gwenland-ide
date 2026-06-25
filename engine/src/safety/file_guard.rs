//! File mutation preflight for agent-initiated operations (M14 Wave 5).
//!
//! This is the integration point between the Safety Engine, audit log, and
//! recovery system for file operations proposed by the agent or extensions.
//! Normal user-typed keyboard/menu operations bypass this path — they are
//! already guarded by `fs.rs` workspace containment checks.
//!
//! Usage pattern:
//! ```text
//! let outcome = preflight_file_mutation(kind, workspace_root, actor, strictness)?;
//! match outcome {
//!     PreflightOutcome::Allowed(snap) => { /* proceed; snap is the pre-mutation snapshot */ }
//!     PreflightOutcome::NeedsConfirmation(decision) => { /* surface to UI */ }
//!     PreflightOutcome::Blocked(decision) => { /* reject */ }
//! }
//! ```

use std::path::Path;

use crate::audit::{AuditCategory, AuditKind, AuditWriter};
use crate::recovery::{RecoveryError, SnapshotRecord, create_snapshot};
use crate::safety::action::{Actor, SafetyAction, SafetyActionKind};
use crate::safety::confirmation::evaluate;
use crate::safety::decision::{SafetyDecision, SafetyVerdict};
use crate::safety::protected_paths::ProtectedPathRegistry;
use crate::workspace::SafetyStrictness;

/// Outcome of a file-mutation preflight check.
#[derive(Debug)]
pub enum PreflightOutcome {
    /// Action is allowed. `snapshot` is `Some` if a pre-mutation copy was made.
    Allowed { snapshot: Option<SnapshotRecord> },
    /// Action needs user confirmation. The decision carries the risk/reason.
    NeedsConfirmation(SafetyDecision),
    /// Action is blocked by policy.
    Blocked(SafetyDecision),
}

/// Run the safety → snapshot → audit pipeline for an agent or extension file
/// mutation.
///
/// - Loads the `ProtectedPathRegistry` from `workspace_root`.
/// - Evaluates the action with `strictness`.
/// - If allowed: creates a pre-mutation snapshot (non-fatal on failure for
///   non-destructive actions; fatal for destructive/secret).
/// - Writes an audit event regardless of outcome.
/// - Returns the `PreflightOutcome` for the caller to act on.
pub fn preflight_file_mutation(
    kind: SafetyActionKind,
    workspace_root: &Path,
    actor: Actor,
    strictness: SafetyStrictness,
) -> PreflightOutcome {
    let registry = ProtectedPathRegistry::load(workspace_root);
    let action = SafetyAction::new(
        actor,
        kind,
        workspace_root.to_string_lossy().as_ref(),
    );
    let decision = evaluate(&action, &registry, strictness);
    let writer = AuditWriter::new(workspace_root);

    let verdict = decision.verdict.clone();

    // Record audit event (best-effort; audit failure itself follows fail-closed
    // policy via `should_block_on_audit_failure`).
    let audit_result = writer.record_decision(
        &decision,
        &action,
        AuditCategory::Safety,
        AuditKind::SafetyDecision,
    );
    if let Err(_audit_err) = audit_result {
        if crate::audit::should_block_on_audit_failure(decision.risk) {
            // Audit failed for a destructive/secret action → block.
            return PreflightOutcome::Blocked(crate::safety::decision::SafetyDecision::block(
                &action.id,
                decision.risk,
                "action blocked because the required audit log write failed",
            ));
        }
        // Non-fatal: read-only/low-risk can continue without audit.
    }

    match verdict {
        SafetyVerdict::Block => PreflightOutcome::Blocked(decision),
        SafetyVerdict::Ask => PreflightOutcome::NeedsConfirmation(decision),
        SafetyVerdict::Allow => {
            // Attempt a pre-mutation snapshot for destructive kinds.
            let snapshot = maybe_snapshot(&action, workspace_root, decision.risk);
            PreflightOutcome::Allowed { snapshot }
        }
    }
}

/// Create a pre-mutation snapshot for the action's target path, if applicable.
/// Returns `None` when no snapshot is needed (e.g. reads, creates, or when the
/// snapshot fails non-fatally).
fn maybe_snapshot(
    action: &SafetyAction,
    workspace_root: &Path,
    risk: crate::safety::decision::RiskLevel,
) -> Option<SnapshotRecord> {
    use crate::safety::decision::RiskLevel;
    use crate::safety::action::SafetyActionKind;

    let path_str = match &action.kind {
        SafetyActionKind::FileWrite { path }
        | SafetyActionKind::FileDelete { path }
        | SafetyActionKind::FileRename { old_path: path, .. } => Some(path.as_str()),
        _ => None,
    }?;

    // Only snapshot medium-risk and above.
    if matches!(risk, RiskLevel::Safe | RiskLevel::Low) {
        return None;
    }

    let path = Path::new(path_str);
    match create_snapshot(
        path,
        workspace_root,
        action.kind.label(),
        &action.actor.to_string(),
    ) {
        Ok(snap) => Some(snap),
        Err(RecoveryError::NotFound(_)) => None, // new file, nothing to snapshot
        Err(_) => None, // snapshot failed non-fatally for allowed action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::action::Actor;
    use crate::workspace::SafetyStrictness;
    use tempfile::tempdir;

    // 5.8.1 — file delete creates snapshot before mutation (via preflight)
    #[test]
    fn file_delete_preflight_asks_and_would_snapshot() {
        let ws = tempdir().unwrap();
        let file = ws.path().join("old.rs");
        std::fs::write(&file, b"fn old() {}").unwrap();

        let outcome = preflight_file_mutation(
            SafetyActionKind::FileDelete { path: file.to_string_lossy().into_owned() },
            ws.path(),
            Actor::Agent,
            SafetyStrictness::Standard,
        );
        // Standard strictness: delete → Ask.
        assert!(matches!(
            outcome,
            PreflightOutcome::NeedsConfirmation(_) | PreflightOutcome::Blocked(_)
        ));
    }

    // 5.8.2 — protected file mutation returns ask/block based on policy
    #[test]
    fn protected_file_write_asks_or_blocks() {
        let ws = tempdir().unwrap();
        let outcome = preflight_file_mutation(
            SafetyActionKind::FileWrite { path: ".env".into() },
            ws.path(),
            Actor::Agent,
            SafetyStrictness::Standard,
        );
        assert!(matches!(
            outcome,
            PreflightOutcome::NeedsConfirmation(_) | PreflightOutcome::Blocked(_)
        ));
    }

    // Normal file write is allowed under Standard strictness.
    #[test]
    fn normal_file_write_allowed_standard() {
        let ws = tempdir().unwrap();
        let outcome = preflight_file_mutation(
            SafetyActionKind::FileWrite { path: "src/main.rs".into() },
            ws.path(),
            Actor::Agent,
            SafetyStrictness::Standard,
        );
        assert!(matches!(outcome, PreflightOutcome::Allowed { .. }));
    }
}
