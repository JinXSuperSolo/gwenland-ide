//! Safety action DTOs — what is being attempted, by whom, on what.
//!
//! Every field here must be secret-safe (no file contents, no API keys, no
//! full terminal output). Target summaries are short human-readable strings
//! that can safely appear in audit logs and confirmation dialogs.

use serde::{Deserialize, Serialize};

/// Who initiated the action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    /// The human user (keyboard, mouse, menu).
    User,
    /// The AI agent (M10 agentic loop).
    Agent,
    /// An extension (M14 Wave 6 foundation).
    Extension { id: String },
    /// An internal IDE subsystem.
    System,
}

impl std::fmt::Display for Actor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Actor::User => write!(f, "user"),
            Actor::Agent => write!(f, "agent"),
            Actor::Extension { id } => write!(f, "extension:{id}"),
            Actor::System => write!(f, "system"),
        }
    }
}

/// The kind of action being evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SafetyActionKind {
    // --- File operations ---
    FileRead { path: String },
    FileCreate { path: String },
    FileWrite { path: String },
    FileDelete { path: String },
    FileRename { old_path: String, new_path: String },
    FileCopy { src: String, dest: String },

    // --- Terminal operations ---
    /// A command proposed for execution (by agent or extension).
    TerminalCommand { command: String },

    // --- Git operations ---
    GitRead,
    GitCommit { message_summary: String },
    GitCheckout { target: String },
    GitBranchDelete { branch: String },
    /// Destructive: reset --hard, clean, force-push, etc.
    GitDestructive { summary: String },
    /// Remote network operation: push/pull/fetch.
    GitRemote { summary: String },

    // --- AI context boundary ---
    /// A file or set of files is about to be included in an AI context window.
    AiContextInclude { path_count: usize, has_secret_path: bool },
    /// A full AI response is about to be stored/persisted locally.
    AiResponseStore,

    // --- Extension operations ---
    ExtensionPermission { extension_id: String, permission: String },

    // --- Remote / export operations ---
    RemoteExport { destination_summary: String },

    // --- Catch-all for unknown or future action kinds ---
    Unknown { summary: String },
}

impl SafetyActionKind {
    /// Short human-readable label for audit logs and confirmation dialogs.
    pub fn label(&self) -> &'static str {
        match self {
            Self::FileRead { .. } => "file_read",
            Self::FileCreate { .. } => "file_create",
            Self::FileWrite { .. } => "file_write",
            Self::FileDelete { .. } => "file_delete",
            Self::FileRename { .. } => "file_rename",
            Self::FileCopy { .. } => "file_copy",
            Self::TerminalCommand { .. } => "terminal_command",
            Self::GitRead => "git_read",
            Self::GitCommit { .. } => "git_commit",
            Self::GitCheckout { .. } => "git_checkout",
            Self::GitBranchDelete { .. } => "git_branch_delete",
            Self::GitDestructive { .. } => "git_destructive",
            Self::GitRemote { .. } => "git_remote",
            Self::AiContextInclude { .. } => "ai_context_include",
            Self::AiResponseStore => "ai_response_store",
            Self::ExtensionPermission { .. } => "extension_permission",
            Self::RemoteExport { .. } => "remote_export",
            Self::Unknown { .. } => "unknown",
        }
    }
}

/// A complete safety action request: who is doing what, on what target, in
/// which workspace. This is the input to the Safety Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyAction {
    /// Unique id for this evaluation (used to correlate audit events).
    pub id: String,
    pub actor: Actor,
    pub kind: SafetyActionKind,
    /// Absolute path of the workspace root (for protected-path resolution).
    pub workspace_root: String,
    /// Optional caller-supplied correlation id (e.g. agent session id).
    #[serde(default)]
    pub correlation_id: Option<String>,
}

impl SafetyAction {
    pub fn new(
        actor: Actor,
        kind: SafetyActionKind,
        workspace_root: impl Into<String>,
    ) -> Self {
        Self {
            id: crate::agentic::new_id(),
            actor,
            kind,
            workspace_root: workspace_root.into(),
            correlation_id: None,
        }
    }

    pub fn with_correlation(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }
}
