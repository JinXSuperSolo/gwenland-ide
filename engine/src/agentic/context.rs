//! Context preview model (M10, Requirement 3).
//!
//! Pure DTOs describing what context the agent *would* send to the provider,
//! plus byte budgets and safe defaults. The actual gathering (reading files,
//! pulling open-tab/diagnostic state) happens Tauri-side, but every candidate is
//! filtered through [`crate::agentic::policy`] and these budgets before it can be
//! marked `included`. No Tauri/UI types appear here.

use serde::{Deserialize, Serialize};

use crate::agentic::policy;

/// Max number of context items shown/sent at once.
pub const MAX_CONTEXT_ITEMS: usize = 24;
/// Max bytes for a single context item's content before it is omitted as oversized.
pub const MAX_ITEM_BYTES: usize = 64 * 1024;
/// Max total included bytes across the whole preview.
pub const MAX_TOTAL_CONTEXT_BYTES: usize = 256 * 1024;

/// What a context item represents (Requirement 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextItemKind {
    ActiveFile,
    Selection,
    OpenTab,
    Diagnostic,
    TerminalError,
    File,
    WorkspaceTree,
}

/// One candidate context item. `included` reflects the current send decision;
/// `redacted` notes that inline secrets were scrubbed from `content`. `reason`
/// is a short human note (e.g. "active editor file", "removed by user").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextItem {
    pub id: String,
    pub kind: ContextItemKind,
    pub path: Option<String>,
    pub label: String,
    pub content: Option<String>,
    pub byte_len: usize,
    pub included: bool,
    pub redacted: bool,
    pub reason: String,
}

impl ContextItem {
    /// Build an included item from already-gathered content, scrubbing inline
    /// secrets and recording byte length. Callers must have already verified the
    /// path is not secret/excluded (use [`omission_for_path`]).
    pub fn included(
        id: impl Into<String>,
        kind: ContextItemKind,
        path: Option<String>,
        label: impl Into<String>,
        content: Option<String>,
        reason: impl Into<String>,
    ) -> Self {
        let (content, redacted) = match content {
            Some(c) => {
                let (scrubbed, hit) = policy::redact_secrets(&c);
                (Some(scrubbed), hit)
            }
            None => (None, false),
        };
        let byte_len = content.as_deref().map(str::len).unwrap_or(0);
        Self {
            id: id.into(),
            kind,
            path,
            label: label.into(),
            content,
            byte_len,
            included: true,
            redacted,
            reason: reason.into(),
        }
    }
}

/// Why a candidate was kept out of the provider request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmissionReason {
    /// Matched a secret denylist pattern (Req 3.2/3.3).
    SecretPath,
    /// In a generated/dependency/build/VCS folder (Req 3.5).
    Excluded,
    /// Content exceeded the per-item or total byte budget.
    Oversized,
    /// Not valid UTF-8 text (Req 3.5).
    Binary,
    /// The file could not be read.
    ReadError,
    /// Resolved outside the workspace root (Req 3.4).
    OutsideWorkspace,
    /// The user explicitly removed it from the preview (Req 3.7).
    UserRemoved,
}

impl OmissionReason {
    pub fn note(self) -> &'static str {
        match self {
            OmissionReason::SecretPath => "omitted: matches a secret pattern",
            OmissionReason::Excluded => "omitted: generated/dependency/build folder",
            OmissionReason::Oversized => "omitted: exceeds size budget",
            OmissionReason::Binary => "omitted: not UTF-8 text",
            OmissionReason::ReadError => "omitted: could not be read",
            OmissionReason::OutsideWorkspace => "omitted: outside the workspace",
            OmissionReason::UserRemoved => "removed by you",
        }
    }
}

/// A candidate that was not included, with a user-safe explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextOmission {
    pub path: String,
    pub label: String,
    pub reason: OmissionReason,
    /// Short extra detail (never secret contents).
    pub detail: String,
}

impl ContextOmission {
    pub fn new(path: impl Into<String>, label: impl Into<String>, reason: OmissionReason) -> Self {
        Self {
            path: path.into(),
            label: label.into(),
            reason,
            detail: reason.note().to_string(),
        }
    }
}

/// The full context preview shown before a provider request (Requirement 3.6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPreview {
    pub items: Vec<ContextItem>,
    pub total_bytes: usize,
    pub omitted: Vec<ContextOmission>,
}

impl ContextPreview {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sum of byte lengths of currently-included items.
    pub fn included_bytes(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.included)
            .map(|i| i.byte_len)
            .sum()
    }

    /// Recompute `total_bytes` from included items.
    pub fn recompute_total(&mut self) {
        self.total_bytes = self.included_bytes();
    }

    /// Ids of currently-included items.
    pub fn included_ids(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|i| i.included)
            .map(|i| i.id.clone())
            .collect()
    }
}

/// Decide whether a path-bearing candidate must be omitted purely on path
/// grounds (before any read). Returns the omission reason, or `None` if the path
/// is allowed. Oversize/binary/read decisions happen at read time.
pub fn omission_for_path(path: &str) -> Option<OmissionReason> {
    if policy::is_secret_path(path) {
        Some(OmissionReason::SecretPath)
    } else if policy::is_excluded_path(path) {
        Some(OmissionReason::Excluded)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_and_excluded_paths_are_omitted() {
        assert_eq!(omission_for_path(".env"), Some(OmissionReason::SecretPath));
        assert_eq!(
            omission_for_path("server.pem"),
            Some(OmissionReason::SecretPath)
        );
        assert_eq!(
            omission_for_path("node_modules/x.js"),
            Some(OmissionReason::Excluded)
        );
        assert_eq!(omission_for_path("src/main.rs"), None);
    }

    #[test]
    fn included_item_redacts_inline_secrets() {
        let item = ContextItem::included(
            "i1",
            ContextItemKind::File,
            Some("src/config.ts".into()),
            "config.ts",
            Some("const KEY = 'sk-ant-abcdefghijklmnopqrstuvwxyz0123456789'".into()),
            "attached",
        );
        assert!(item.redacted);
        assert!(item.content.unwrap().contains("[REDACTED]"));
    }

    #[test]
    fn preview_byte_accounting_counts_only_included() {
        let mut preview = ContextPreview::new();
        let mut a = ContextItem::included(
            "a",
            ContextItemKind::File,
            None,
            "a",
            Some("hello".into()),
            "x",
        );
        let mut b = ContextItem::included(
            "b",
            ContextItemKind::File,
            None,
            "b",
            Some("world!".into()),
            "x",
        );
        a.included = true;
        b.included = false;
        preview.items.push(a);
        preview.items.push(b);
        preview.recompute_total();
        assert_eq!(preview.total_bytes, 5); // only "hello"
        assert_eq!(preview.included_ids(), vec!["a".to_string()]);
    }
}
