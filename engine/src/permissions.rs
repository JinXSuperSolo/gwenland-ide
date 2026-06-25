//! Extension permission foundation (M14 Wave 6).
//!
//! Defines the local extension permission registry, the default permission
//! matrix, approval history, and integration with the Safety Engine.
//!
//! All state is stored locally:
//! - Permission registry → `.gwenland/extensions/permissions.json`
//! - Approval history   → `.gwenland/extensions/approvals.jsonl`
//!
//! No extension runtime is implemented in M14; this is the permission
//! substrate that a future extension host must use.
//!
//! Default permission matrix (Requirement 12.5):
//! | Permission       | Default  |
//! |-----------------|----------|
//! | read_workspace  | allowed  |
//! | write_file      | ask      |
//! | delete_file     | blocked  |
//! | run_terminal    | ask      |
//! | access_git      | ask      |
//! | access_env      | blocked  |
//! | access_database | blocked  |
//! | <unknown>       | blocked  |

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Permission enum
// ---------------------------------------------------------------------------

/// Known extension permission kinds (Requirement 12.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ReadWorkspace,
    WriteFile,
    DeleteFile,
    RunTerminal,
    AccessGit,
    AccessEnv,
    AccessDatabase,
    /// Any permission not in this list.
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::ReadWorkspace => "read_workspace",
            Self::WriteFile => "write_file",
            Self::DeleteFile => "delete_file",
            Self::RunTerminal => "run_terminal",
            Self::AccessGit => "access_git",
            Self::AccessEnv => "access_env",
            Self::AccessDatabase => "access_database",
            Self::Unknown => "unknown",
        };
        write!(f, "{s}")
    }
}

impl Permission {
    pub fn from_str(s: &str) -> Self {
        match s {
            "read_workspace" => Self::ReadWorkspace,
            "write_file" => Self::WriteFile,
            "delete_file" => Self::DeleteFile,
            "run_terminal" => Self::RunTerminal,
            "access_git" => Self::AccessGit,
            "access_env" => Self::AccessEnv,
            "access_database" => Self::AccessDatabase,
            _ => Self::Unknown,
        }
    }
}

// ---------------------------------------------------------------------------
// Permission decision
// ---------------------------------------------------------------------------

/// The default policy verdict for a permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDefault {
    /// Always allowed without asking.
    Allowed,
    /// Requires explicit user approval each time (or a stored approval).
    Ask,
    /// Blocked; requires explicit danger acknowledgment to override.
    Blocked,
}

impl std::fmt::Display for PermissionDefault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allowed => write!(f, "allowed"),
            Self::Ask => write!(f, "ask"),
            Self::Blocked => write!(f, "blocked"),
        }
    }
}

/// The resolved permission decision for a specific extension+permission pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub extension_id: String,
    pub permission: String,
    pub verdict: PermissionDefault,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Default permission matrix
// ---------------------------------------------------------------------------

/// Return the default `PermissionDefault` for a `Permission` (Requirement 12.5).
pub fn default_for_permission(perm: &Permission) -> PermissionDefault {
    match perm {
        Permission::ReadWorkspace => PermissionDefault::Allowed,
        Permission::WriteFile => PermissionDefault::Ask,
        Permission::DeleteFile => PermissionDefault::Blocked,
        Permission::RunTerminal => PermissionDefault::Ask,
        Permission::AccessGit => PermissionDefault::Ask,
        Permission::AccessEnv => PermissionDefault::Blocked,
        Permission::AccessDatabase => PermissionDefault::Blocked,
        Permission::Unknown => PermissionDefault::Blocked,
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Per-extension permission entry in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionPermissionEntry {
    pub extension_id: String,
    /// Overrides for specific permissions. Absent permissions use the default matrix.
    pub overrides: std::collections::BTreeMap<String, PermissionDefault>,
}

/// The full permission registry loaded from (or defaulting to) the workspace store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionRegistry {
    pub extensions: Vec<ExtensionPermissionEntry>,
}

impl PermissionRegistry {
    /// Path of the registry file.
    fn registry_path(workspace_root: &Path) -> PathBuf {
        crate::workspace::extensions_dir(workspace_root).join("permissions.json")
    }

    /// Path of the approval history JSONL.
    fn approvals_path(workspace_root: &Path) -> PathBuf {
        crate::workspace::extensions_dir(workspace_root).join("approvals.jsonl")
    }

    /// Load the registry from `.gwenland/extensions/permissions.json`, falling
    /// back to an empty (all-default) registry when absent or malformed.
    pub fn load(workspace_root: &Path) -> Self {
        let path = Self::registry_path(workspace_root);
        if !path.exists() {
            return Self::default();
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) if !c.trim().is_empty() => c,
            _ => return Self::default(),
        };
        serde_json::from_str(&content).unwrap_or_default()
    }

    /// Save the registry atomically.
    pub fn save(&self, workspace_root: &Path) -> Result<(), PermissionError> {
        let dir = crate::workspace::extensions_dir(workspace_root);
        std::fs::create_dir_all(&dir)?;
        let path = Self::registry_path(workspace_root);
        let tmp = path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, &content)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Resolve the effective permission verdict for `extension_id` + `permission`.
    /// Applies per-extension overrides over the default matrix.
    pub fn resolve(&self, extension_id: &str, permission: &Permission) -> PermissionDecision {
        let default_verdict = default_for_permission(permission);
        let perm_str = permission.to_string();

        let verdict = self
            .extensions
            .iter()
            .find(|e| e.extension_id == extension_id)
            .and_then(|e| e.overrides.get(&perm_str))
            .copied()
            .unwrap_or(default_verdict);

        let reason = match verdict {
            PermissionDefault::Allowed => format!("{perm_str}: allowed by default matrix"),
            PermissionDefault::Ask => format!("{perm_str}: requires explicit approval"),
            PermissionDefault::Blocked => format!("{perm_str}: blocked by default policy"),
        };

        PermissionDecision {
            extension_id: extension_id.to_string(),
            permission: perm_str,
            verdict,
            reason,
        }
    }
}

// ---------------------------------------------------------------------------
// Approval history
// ---------------------------------------------------------------------------

/// One approval (or denial) record stored in the approvals JSONL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub id: String,
    pub timestamp: String,
    pub extension_id: String,
    pub permission: String,
    pub approved: bool,
    /// Bounded, redacted description of what the extension requested.
    #[serde(default)]
    pub target_summary: String,
}

impl ApprovalRecord {
    pub fn new(
        extension_id: impl Into<String>,
        permission: impl Into<String>,
        approved: bool,
        target_summary: impl Into<String>,
    ) -> Self {
        use crate::agentic::policy::redact_secrets;
        let raw = target_summary.into();
        let (redacted, _) = redact_secrets(&raw);
        let bounded = if redacted.len() > 256 {
            format!("{}…", &redacted[..256])
        } else {
            redacted
        };
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            extension_id: extension_id.into(),
            permission: permission.into(),
            approved,
            target_summary: bounded,
        }
    }
}

/// Append one approval record to the workspace approvals JSONL.
pub fn record_approval(
    workspace_root: &Path,
    record: &ApprovalRecord,
) -> Result<(), PermissionError> {
    let dir = crate::workspace::extensions_dir(workspace_root);
    std::fs::create_dir_all(&dir)?;
    let path = PermissionRegistry::approvals_path(workspace_root);
    let mut line = serde_json::to_string(record)?;
    line.push('\n');
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    f.write_all(line.as_bytes())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Safety Engine integration
// ---------------------------------------------------------------------------

/// Convert a permission request into a `SafetyDecision` via the Safety Engine.
/// This is how Wave 6 integrates with Wave 2 — the permission system is not a
/// separate policy silo; it delegates to the Safety Engine.
pub fn evaluate_permission(
    extension_id: &str,
    permission: &Permission,
    workspace_root: &Path,
) -> crate::safety::SafetyDecision {
    use crate::safety::action::{Actor, SafetyAction, SafetyActionKind};
    use crate::safety::protected_paths::ProtectedPathRegistry;
    use crate::workspace::SafetyStrictness;

    let kind = SafetyActionKind::ExtensionPermission {
        extension_id: extension_id.to_string(),
        permission: permission.to_string(),
    };
    let action = SafetyAction::new(
        Actor::Extension {
            id: extension_id.to_string(),
        },
        kind,
        workspace_root.to_string_lossy().as_ref(),
    );
    let registry = ProtectedPathRegistry::load(workspace_root);
    crate::safety::evaluate(&action, &registry, SafetyStrictness::Standard)
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum PermissionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::SafetyVerdict;
    use tempfile::tempdir;

    // 6.6.1 — default matrix matches requirements
    #[test]
    fn default_matrix_matches_requirements() {
        assert_eq!(default_for_permission(&Permission::ReadWorkspace), PermissionDefault::Allowed);
        assert_eq!(default_for_permission(&Permission::WriteFile), PermissionDefault::Ask);
        assert_eq!(default_for_permission(&Permission::DeleteFile), PermissionDefault::Blocked);
        assert_eq!(default_for_permission(&Permission::RunTerminal), PermissionDefault::Ask);
        assert_eq!(default_for_permission(&Permission::AccessGit), PermissionDefault::Ask);
        assert_eq!(default_for_permission(&Permission::AccessEnv), PermissionDefault::Blocked);
        assert_eq!(default_for_permission(&Permission::AccessDatabase), PermissionDefault::Blocked);
    }

    // 6.6.2 — unknown permissions are blocked
    #[test]
    fn unknown_permissions_are_blocked() {
        assert_eq!(
            default_for_permission(&Permission::Unknown),
            PermissionDefault::Blocked
        );
        // Via string parsing.
        assert_eq!(
            default_for_permission(&Permission::from_str("hack_the_planet")),
            PermissionDefault::Blocked
        );
    }

    // 6.6.3 — registry round-trips locally
    #[test]
    fn registry_round_trips_locally() {
        let dir = tempdir().unwrap();
        let mut reg = PermissionRegistry::default();
        reg.extensions.push(ExtensionPermissionEntry {
            extension_id: "my-ext".to_string(),
            overrides: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("read_workspace".to_string(), PermissionDefault::Allowed);
                m
            },
        });
        reg.save(dir.path()).unwrap();
        let loaded = PermissionRegistry::load(dir.path());
        assert_eq!(loaded.extensions.len(), 1);
        assert_eq!(loaded.extensions[0].extension_id, "my-ext");
    }

    // 6.6.4 — malformed registry falls back to defaults
    #[test]
    fn malformed_registry_falls_back_to_defaults() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".gwenland/extensions")).unwrap();
        std::fs::write(
            dir.path().join(".gwenland/extensions/permissions.json"),
            b"not-json",
        )
        .unwrap();
        let reg = PermissionRegistry::load(dir.path());
        assert!(reg.extensions.is_empty(), "malformed → empty default registry");
    }

    // 6.6.5 — approval history appends and redacts target summaries
    #[test]
    fn approval_history_appends_and_redacts() {
        let dir = tempdir().unwrap();
        let rec1 = ApprovalRecord::new("ext-a", "write_file", true, "src/main.rs");
        let rec2 = ApprovalRecord::new(
            "ext-a",
            "write_file",
            false,
            "secret=sk-ant-abcdefghijklmnopqrstuvwxyz01234",
        );
        record_approval(dir.path(), &rec1).unwrap();
        record_approval(dir.path(), &rec2).unwrap();

        let path = dir.path().join(".gwenland/extensions/approvals.jsonl");
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        // Second record must have secret redacted.
        assert!(
            !content.contains("sk-ant-abcdefghijklmnopq"),
            "secret must not appear in approval history"
        );
        assert!(content.contains("[REDACTED]"), "redaction must be present");
    }

    // Safety Engine integration: read_workspace → Allow; delete_file → Block
    #[test]
    fn safety_engine_integration_correct() {
        let dir = tempdir().unwrap();
        let allow = evaluate_permission("ext-a", &Permission::ReadWorkspace, dir.path());
        assert_eq!(allow.verdict, SafetyVerdict::Allow);

        let block = evaluate_permission("ext-a", &Permission::DeleteFile, dir.path());
        assert_eq!(block.verdict, SafetyVerdict::Block);

        let ask = evaluate_permission("ext-a", &Permission::WriteFile, dir.path());
        assert_eq!(ask.verdict, SafetyVerdict::Ask);
    }

    // Registry resolve uses overrides over defaults.
    #[test]
    fn registry_resolve_applies_overrides() {
        let mut reg = PermissionRegistry::default();
        reg.extensions.push(ExtensionPermissionEntry {
            extension_id: "my-ext".to_string(),
            overrides: {
                let mut m = std::collections::BTreeMap::new();
                // Override write_file to Allowed for this specific extension.
                m.insert("write_file".to_string(), PermissionDefault::Allowed);
                m
            },
        });
        let decision = reg.resolve("my-ext", &Permission::WriteFile);
        assert_eq!(decision.verdict, PermissionDefault::Allowed);

        // Other extensions still get the default.
        let default_decision = reg.resolve("other-ext", &Permission::WriteFile);
        assert_eq!(default_decision.verdict, PermissionDefault::Ask);
    }

    // No tmp artefact after save.
    #[test]
    fn no_tmp_artefact_after_registry_save() {
        let dir = tempdir().unwrap();
        PermissionRegistry::default().save(dir.path()).unwrap();
        let tmp = dir.path().join(".gwenland/extensions/permissions.json.tmp");
        assert!(!tmp.exists());
    }
}
