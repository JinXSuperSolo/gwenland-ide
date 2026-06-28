//! Safety Engine (M14 Wave 2).
//!
//! Local safety classification and gating for file, terminal, Git, AI-context,
//! and extension actions. No Tauri, UI, or browser imports. No cloud dependency.
//! Policy is fail-conservative: unknown actions produce `Ask` or `Block`.
//!
//! ## Public surface
//!
//! - [`action`] — `SafetyAction`, `SafetyActionKind`, `Actor`
//! - [`decision`] — `SafetyDecision`, `SafetyVerdict`, `RiskLevel`, `ConfirmationKind`
//! - [`protected_paths`] — `ProtectedPathRegistry` (load / defaults / classify)
//! - [`confirmation`] — `evaluate(action, registry, strictness)` — the single policy fn
//! - [`guards`] — high-level convenience helpers for common engine callsites
//!
//! Redaction lives in `crate::agentic::policy::redact_secrets` (M10); M14 reuses
//! it rather than duplicating it.

pub mod action;
pub mod audit;
pub mod confirmation;
pub mod decision;
pub mod file_guard;
pub mod guards;
pub mod history;
pub mod permissions;
pub mod protected_paths;
pub mod recovery;
pub mod search_policy;

pub use action::{Actor, SafetyAction, SafetyActionKind};
pub use confirmation::evaluate;
pub use decision::{ConfirmationKind, RiskLevel, SafetyDecision, SafetyStrictness, SafetyVerdict};
pub use file_guard::{PreflightOutcome, preflight_file_mutation};
pub use guards::{check_ai_context, check_file_action, check_terminal_command, file_write_allowed};
pub use protected_paths::{ProtectedPathEntry, ProtectedPathRegistry, ProtectionLevel};
