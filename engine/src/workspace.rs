//! Local-first workspace store: `.gwenland/` path helpers and per-workspace
//! settings overlay (M14 Wave 1).
//!
//! Design rules:
//! - All paths resolve inside `<workspace_root>/.gwenland/`. No helper ever
//!   returns a path outside the workspace.
//! - `WorkspaceSettings` uses `#[serde(default)]` on every field so old or
//!   missing files always deserialize without error.
//! - API keys and secrets are explicitly absent from the model.
//! - Load failures return `Default::default()` (fail-open for read-only data).
//! - Saves are atomic: write to a `.tmp` sibling, then rename over the target.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Returns the `.gwenland/` directory inside `workspace_root` without creating
/// it. Callers that write must create it themselves (`ensure_gwenland_dir`).
pub fn gwenland_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".gwenland")
}

/// Create the `.gwenland/` directory (and any parents) if it does not exist.
pub fn ensure_gwenland_dir(workspace_root: &Path) -> std::io::Result<PathBuf> {
    let dir = gwenland_dir(workspace_root);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// `.gwenland/settings.json`
pub fn settings_path(workspace_root: &Path) -> PathBuf {
    gwenland_dir(workspace_root).join("settings.json")
}

/// `.gwenland/safety/`
pub fn safety_dir(workspace_root: &Path) -> PathBuf {
    gwenland_dir(workspace_root).join("safety")
}

/// `.gwenland/snapshots/`
pub fn snapshots_dir(workspace_root: &Path) -> PathBuf {
    gwenland_dir(workspace_root).join("snapshots")
}

/// `.gwenland/trash/`
pub fn trash_dir(workspace_root: &Path) -> PathBuf {
    gwenland_dir(workspace_root).join("trash")
}

/// `.gwenland/backups/`
pub fn backups_dir(workspace_root: &Path) -> PathBuf {
    gwenland_dir(workspace_root).join("backups")
}

/// `.gwenland/audit/`
pub fn audit_dir(workspace_root: &Path) -> PathBuf {
    gwenland_dir(workspace_root).join("audit")
}

/// `.gwenland/agent/` (M13 memory lives here)
pub fn agent_dir(workspace_root: &Path) -> PathBuf {
    gwenland_dir(workspace_root).join("agent")
}

/// `.gwenland/extensions/`
pub fn extensions_dir(workspace_root: &Path) -> PathBuf {
    gwenland_dir(workspace_root).join("extensions")
}

/// `.gwenland/logs/`
pub fn logs_dir(workspace_root: &Path) -> PathBuf {
    gwenland_dir(workspace_root).join("logs")
}

/// Canonicalize `workspace_root` and return it, or an error if it does not
/// exist / cannot be resolved. Used at workspace-open time to pin the root.
pub fn canonical_workspace_root(workspace_root: &Path) -> Result<PathBuf, WorkspaceError> {
    workspace_root
        .canonicalize()
        .map_err(|_| WorkspaceError::InvalidRoot(workspace_root.to_string_lossy().into_owned()))
}

/// Returns `true` iff `path` resolves strictly inside `workspace_root`
/// (the root itself is not considered "inside").
pub fn is_inside_workspace(path: &Path, workspace_root: &Path) -> bool {
    let Ok(root) = workspace_root.canonicalize() else {
        return false;
    };
    let target = if path.exists() {
        match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        }
    } else {
        // For not-yet-existing paths, canonicalize the deepest existing ancestor
        // and re-append the tail (rejecting any `..` escape).
        match resolve_nonexistent(path) {
            Some(p) => p,
            None => return false,
        }
    };
    target != root && target.starts_with(&root)
}

/// Resolve a not-yet-existing path: canonicalize the deepest existing ancestor
/// and fold the remaining components on. Returns `None` if the path cannot be
/// made safe (any `..` that would escape, missing root, etc.).
fn resolve_nonexistent(path: &Path) -> Option<PathBuf> {
    let mut base = path;
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if base.exists() {
            let mut resolved = base.canonicalize().ok()?;
            for component in tail.into_iter().rev() {
                resolved.push(component);
            }
            return Some(resolved);
        }
        let name = base.file_name()?.to_owned();
        tail.push(name);
        base = base.parent()?;
    }
}

// ---------------------------------------------------------------------------
// WorkspaceSettings model
// ---------------------------------------------------------------------------

/// Safety strictness level: how aggressively to gate risky actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SafetyStrictness {
    /// Ask for confirmation on medium-risk actions; block destructive/secret.
    #[default]
    Standard,
    /// Ask on low-risk; block medium/high/destructive/secret.
    Strict,
    /// Block everything except explicitly safe reads.
    Paranoid,
}

/// Per-workspace settings overlay. Stored under `.gwenland/settings.json`.
/// Merged over global settings at runtime; absent fields inherit global values.
///
/// **No API keys, tokens, passwords, or credentials may appear here.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkspaceSettings {
    /// Theme override for this workspace ("dark" | "light" | "system").
    #[serde(default)]
    pub theme: Option<String>,
    /// Accent color override (CSS hex string, e.g. "#7c3aed").
    #[serde(default)]
    pub accent_color: Option<String>,
    /// Editor font family override.
    #[serde(default)]
    pub editor_font: Option<String>,
    /// Terminal font family override.
    #[serde(default)]
    pub terminal_font: Option<String>,
    /// Serialized layout state blob (opaque to engine; owned by the UI).
    #[serde(default)]
    pub layout_state: Option<serde_json::Value>,
    /// Whether the sidebar is open.
    #[serde(default)]
    pub sidebar_open: Option<bool>,
    /// Whether the panel (terminal / output) is open.
    #[serde(default)]
    pub panel_open: Option<bool>,
    /// Keybindings overrides (opaque map; UI owns the schema).
    #[serde(default)]
    pub keybindings: Option<serde_json::Value>,
    /// Preferred formatter command (e.g. "prettier", "rustfmt").
    #[serde(default)]
    pub formatter: Option<String>,
    /// Whether to autosave on change.
    #[serde(default)]
    pub autosave: Option<bool>,
    /// Safety strictness for this workspace.
    #[serde(default)]
    pub safety_strictness: Option<SafetyStrictness>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("workspace root cannot be resolved: {0}")]
    InvalidRoot(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Load / save
// ---------------------------------------------------------------------------

/// Load workspace settings from `.gwenland/settings.json` under `workspace_root`.
/// Returns `Default::default()` if the file is absent, empty, or malformed —
/// never errors on missing/corrupt files (fail-open for read-only config data).
pub fn load_workspace_settings(workspace_root: &Path) -> WorkspaceSettings {
    let path = settings_path(workspace_root);
    if !path.exists() {
        return WorkspaceSettings::default();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) if c.trim().is_empty() => return WorkspaceSettings::default(),
        Ok(c) => c,
        Err(_) => return WorkspaceSettings::default(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Save workspace settings to `.gwenland/settings.json` under `workspace_root`,
/// creating the `.gwenland/` directory if needed. Write is atomic (tmp + rename).
pub fn save_workspace_settings(
    workspace_root: &Path,
    settings: &WorkspaceSettings,
) -> Result<(), WorkspaceError> {
    ensure_gwenland_dir(workspace_root)?;
    let path = settings_path(workspace_root);
    let tmp_path = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(&tmp_path, &content)?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // 1.6.1 — path helpers stay inside workspace root
    #[test]
    fn path_helpers_stay_inside_root() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let gw = gwenland_dir(root);
        assert!(gw.starts_with(root));
        assert!(settings_path(root).starts_with(&gw));
        assert!(safety_dir(root).starts_with(&gw));
        assert!(snapshots_dir(root).starts_with(&gw));
        assert!(trash_dir(root).starts_with(&gw));
        assert!(backups_dir(root).starts_with(&gw));
        assert!(audit_dir(root).starts_with(&gw));
        assert!(agent_dir(root).starts_with(&gw));
        assert!(extensions_dir(root).starts_with(&gw));
        assert!(logs_dir(root).starts_with(&gw));
    }

    // 1.6.2 — missing settings returns defaults
    #[test]
    fn missing_settings_returns_defaults() {
        let dir = tempdir().unwrap();
        let s = load_workspace_settings(dir.path());
        assert_eq!(s, WorkspaceSettings::default());
    }

    // 1.6.3 — malformed settings returns defaults
    #[test]
    fn malformed_settings_returns_defaults() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".gwenland")).unwrap();
        std::fs::write(dir.path().join(".gwenland/settings.json"), b"not { json }").unwrap();
        let s = load_workspace_settings(dir.path());
        assert_eq!(s, WorkspaceSettings::default());
    }

    // 1.6.3b — empty file returns defaults
    #[test]
    fn empty_settings_file_returns_defaults() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".gwenland")).unwrap();
        std::fs::write(dir.path().join(".gwenland/settings.json"), b"").unwrap();
        let s = load_workspace_settings(dir.path());
        assert_eq!(s, WorkspaceSettings::default());
    }

    // 1.6.4 — settings round-trip through JSON
    #[test]
    fn settings_round_trip_json() {
        let dir = tempdir().unwrap();
        let mut s = WorkspaceSettings::default();
        s.theme = Some("dark".to_string());
        s.autosave = Some(true);
        s.safety_strictness = Some(SafetyStrictness::Strict);
        s.editor_font = Some("JetBrains Mono".to_string());
        save_workspace_settings(dir.path(), &s).unwrap();
        let loaded = load_workspace_settings(dir.path());
        assert_eq!(s, loaded);
    }

    // 1.6.5 — workspace settings with all None fields round-trips cleanly
    #[test]
    fn default_settings_round_trip() {
        let dir = tempdir().unwrap();
        let s = WorkspaceSettings::default();
        save_workspace_settings(dir.path(), &s).unwrap();
        let loaded = load_workspace_settings(dir.path());
        assert_eq!(s, loaded);
    }

    // 1.6.6 — no tmp artefact after successful save
    #[test]
    fn no_tmp_artefact_after_save() {
        let dir = tempdir().unwrap();
        save_workspace_settings(dir.path(), &WorkspaceSettings::default()).unwrap();
        let tmp = dir.path().join(".gwenland/settings.json.tmp");
        assert!(!tmp.exists(), ".tmp artefact must not remain after save");
    }

    // Secrets: WorkspaceSettings must not contain any secret-bearing fields.
    // This is enforced by the struct definition, but we verify serialization
    // never emits known secret field names.
    #[test]
    fn serialized_settings_contain_no_secret_field_names() {
        let mut s = WorkspaceSettings::default();
        s.formatter = Some("prettier".to_string());
        let json = serde_json::to_string(&s).unwrap();
        for forbidden in ["api_key", "token", "password", "secret", "credential"] {
            assert!(
                !json.contains(forbidden),
                "settings JSON must not contain field name: {forbidden}"
            );
        }
    }

    // is_inside_workspace checks
    #[test]
    fn is_inside_workspace_accepts_children() {
        let dir = tempdir().unwrap();
        let child = dir.path().join("src/main.rs");
        assert!(is_inside_workspace(&child, dir.path()));
    }

    #[test]
    fn is_inside_workspace_rejects_root_itself() {
        let dir = tempdir().unwrap();
        assert!(!is_inside_workspace(dir.path(), dir.path()));
    }

    #[test]
    fn is_inside_workspace_rejects_outside_path() {
        let ws = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let f = outside.path().join("secret.txt");
        std::fs::write(&f, b"x").unwrap();
        assert!(!is_inside_workspace(&f, ws.path()));
    }

    // Workspace overrides: a partial settings file (only some fields set) loads
    // cleanly; absent fields remain None so the UI can fall back to global settings.
    #[test]
    fn partial_settings_load_with_nones_for_missing_fields() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".gwenland")).unwrap();
        std::fs::write(
            dir.path().join(".gwenland/settings.json"),
            br#"{"theme": "light"}"#,
        )
        .unwrap();
        let s = load_workspace_settings(dir.path());
        assert_eq!(s.theme, Some("light".to_string()));
        assert_eq!(s.autosave, None);
        assert_eq!(s.safety_strictness, None);
    }
}
