//! Normalized, user-safe LSP error type (Milestone 6).
//!
//! Every fallible operation in the LSP bridge maps its underlying failure into
//! one of these variants before crossing back toward the Tauri/UI layer. As with
//! [`crate::ai::AiError`], the serde representation is *adjacently tagged*
//! (`{ "kind": ..., "data": ... }`) so the UI can discriminate on `kind`.
//!
//! Critically, no variant must ever carry raw language-server stderr or
//! unescaped control characters: server output is sanitized via [`sanitize`]
//! before it is placed in a `Protocol`/`Crashed` message (Requirement 7.8).

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum LspError {
    /// The configured server command could not be found on disk or in `PATH`.
    /// `command` is the (already user-supplied) command string, safe to show.
    #[error("language server not found: {command}")]
    MissingServer { command: String },

    /// The server is disabled for this language in settings.
    #[error("language server disabled for {language}")]
    Disabled { language: String },

    /// The file extension has no M6 language mapping.
    #[error("unsupported language for this file")]
    UnsupportedLanguage,

    /// Spawning the child process failed (sanitized OS message).
    #[error("failed to start language server: {0}")]
    Spawn(String),

    /// A request was issued before the server finished `initialize`.
    #[error("language server is not initialized")]
    NotInitialized,

    /// The server process exited unexpectedly (sanitized detail).
    #[error("language server crashed: {0}")]
    Crashed(String),

    /// A request did not receive a response within its deadline.
    #[error("language server request timed out")]
    Timeout,

    /// Malformed frame, malformed JSON, or otherwise unparseable protocol
    /// traffic. The string is sanitized and key/secret free.
    #[error("language server protocol error: {0}")]
    Protocol(String),

    /// stdio transport failure (pipe closed, write error). Sanitized message.
    #[error("language server transport error: {0}")]
    Transport(String),
}

impl LspError {
    /// Convenience for protocol-level failures from a sanitizable source.
    pub fn protocol(msg: impl AsRef<str>) -> Self {
        LspError::Protocol(sanitize(msg.as_ref()))
    }

    /// Convenience for transport-level failures from a sanitizable source.
    pub fn transport(msg: impl AsRef<str>) -> Self {
        LspError::Transport(sanitize(msg.as_ref()))
    }
}

/// Strip ASCII control characters (except none — all are removed) and clamp the
/// length so a noisy server cannot blow up a user-visible string. Used on any
/// text that originates from a language server before it lands in an `LspError`
/// (Requirement 7.8).
pub fn sanitize(raw: &str) -> String {
    const MAX: usize = 500;
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control() || *c == ' ')
        .collect();
    if cleaned.chars().count() > MAX {
        let truncated: String = cleaned.chars().take(MAX).collect();
        format!("{truncated}…")
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_error_serde_round_trip_all_variants() {
        let variants = [
            LspError::MissingServer {
                command: "rust-analyzer".into(),
            },
            LspError::Disabled {
                language: "python".into(),
            },
            LspError::UnsupportedLanguage,
            LspError::Spawn("permission denied".into()),
            LspError::NotInitialized,
            LspError::Crashed("exit code 1".into()),
            LspError::Timeout,
            LspError::Protocol("bad frame".into()),
            LspError::Transport("broken pipe".into()),
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let back: LspError = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back, "round-trip mismatch for {json}");
        }
    }

    #[test]
    fn error_serializes_with_kind_tag() {
        let json = serde_json::to_value(LspError::UnsupportedLanguage).unwrap();
        assert_eq!(json["kind"], "unsupported_language");

        let json = serde_json::to_value(LspError::MissingServer {
            command: "pylsp".into(),
        })
        .unwrap();
        assert_eq!(json["kind"], "missing_server");
        assert_eq!(json["data"]["command"], "pylsp");
    }

    #[test]
    fn sanitize_strips_control_chars() {
        let dirty = "panic\u{0007}\n\tat src/main.rs\r";
        let clean = sanitize(dirty);
        assert!(!clean.contains('\u{0007}'));
        assert!(!clean.contains('\n'));
        assert!(!clean.contains('\t'));
        assert!(!clean.contains('\r'));
        assert_eq!(clean, "panicat src/main.rs");
    }

    #[test]
    fn sanitize_clamps_length() {
        let long = "x".repeat(1000);
        let clean = sanitize(&long);
        // 500 chars + the ellipsis.
        assert_eq!(clean.chars().count(), 501);
        assert!(clean.ends_with('…'));
    }
}
