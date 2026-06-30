//! Context attachment expansion (Requirement 14).
//!
//! Turns a user prompt plus its attachments into the single string that is sent
//! to the provider AND persisted to JSONL. Attachments are wrapped in
//! deterministic `<context …>` tags so the model (and future training tooling)
//! sees a stable, parseable structure. Only attribute quotes are escaped (as
//! `&quot;`); file contents, the user prompt, newlines, and code fences are
//! preserved verbatim (Requirement 14.8).
//!
//! File attachments are read here as UTF-8 and validated: binary, missing,
//! directory, unreadable, or out-of-project-root paths are rejected
//! (Requirement 14.6). Count and total byte-size limits are enforced before any
//! provider request (Requirement 14.10).

use std::path::{Path, PathBuf};

use crate::ai::error::AiError;
use crate::ai::provider::ContextAttachment;

/// Max number of attachments per message.
pub const MAX_ATTACHMENTS: usize = 10;
/// Max total bytes across all attachment contents.
pub const MAX_TOTAL_BYTES: usize = 256 * 1024;

fn escape_attr(s: &str) -> String {
    s.replace('"', "&quot;")
}

/// Read + validate a file attachment under `project_root`, returning its text.
fn read_file_attachment(path: &str, project_root: &Path) -> Result<String, AiError> {
    let p = Path::new(path);
    let candidate: PathBuf = if p.is_absolute() {
        p.to_path_buf()
    } else {
        project_root.join(p)
    };
    let canon = std::fs::canonicalize(&candidate)
        .map_err(|_| AiError::ProviderError(format!("cannot read attachment: {path}")))?;
    // Reject paths outside the active project root (Requirement 14.6).
    let root = std::fs::canonicalize(project_root)
        .map_err(|_| AiError::ProviderError("invalid project root".into()))?;
    if !canon.starts_with(&root) {
        return Err(AiError::ProviderError(format!(
            "attachment is outside the project root: {path}"
        )));
    }
    if canon.is_dir() {
        return Err(AiError::ProviderError(format!(
            "cannot attach a directory: {path}"
        )));
    }
    // Reuse the engine's UTF-8/binary-aware reader.
    crate::workspace::fs::read_file(&canon).map_err(|e| match e {
        crate::workspace::fs::FsError::BinaryFile => {
            AiError::ProviderError(format!("cannot attach binary file: {path}"))
        }
        other => AiError::ProviderError(format!("cannot read attachment {path}: {other}")),
    })
}

/// Expand `message` with its `attachments`. Returns the raw message unchanged
/// when there are no attachments (Requirement 14.8). `project_root` bounds file
/// reads.
pub fn expand_message(
    message: &str,
    attachments: &[ContextAttachment],
    project_root: &Path,
) -> Result<String, AiError> {
    if attachments.is_empty() {
        return Ok(message.to_string());
    }
    if attachments.len() > MAX_ATTACHMENTS {
        return Err(AiError::ProviderError(format!(
            "too many attachments ({}, max {MAX_ATTACHMENTS})",
            attachments.len()
        )));
    }

    let mut blocks: Vec<String> = Vec::new();
    let mut total_bytes = 0usize;

    for att in attachments {
        let block = match att {
            ContextAttachment::File { path } => {
                let content = read_file_attachment(path, project_root)?;
                total_bytes += content.len();
                format!(
                    "<context type=\"file\" path=\"{}\">\n{}\n</context>",
                    escape_attr(path),
                    content
                )
            }
            ContextAttachment::Selection { path, content } => {
                if path.trim().is_empty() {
                    return Err(AiError::ProviderError(
                        "selection attachment requires a source path".into(),
                    ));
                }
                total_bytes += content.len();
                format!(
                    "<context type=\"selection\" path=\"{}\">\n{}\n</context>",
                    escape_attr(path),
                    content
                )
            }
            ContextAttachment::TerminalError { label, line } => {
                total_bytes += line.len();
                format!(
                    "<context type=\"terminal_error\" label=\"{}\">\n{}\n</context>",
                    escape_attr(label),
                    line
                )
            }
        };
        blocks.push(block);
    }

    if total_bytes > MAX_TOTAL_BYTES {
        return Err(AiError::ContextLengthExceeded);
    }

    // Context blocks first, then the user's prompt, separated by blank lines.
    Ok(format!("{}\n\n{}", blocks.join("\n\n"), message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn no_attachments_returns_raw_message() {
        let root = tempdir().unwrap();
        let out = expand_message("hello\nworld", &[], root.path()).unwrap();
        assert_eq!(out, "hello\nworld");
    }

    #[test]
    fn file_attachment_is_read_and_wrapped() {
        let root = tempdir().unwrap();
        let file = root.path().join("a.rs");
        fs::write(&file, "fn main() {}\n").unwrap();
        let att = ContextAttachment::File {
            path: file.to_string_lossy().into_owned(),
        };
        let out = expand_message("explain", std::slice::from_ref(&att), root.path()).unwrap();
        assert!(out.contains("<context type=\"file\""));
        assert!(out.contains("fn main() {}"));
        assert!(out.trim_end().ends_with("explain"));
    }

    #[test]
    fn relative_file_attachment_resolves_under_project_root() {
        let root = tempdir().unwrap();
        let src = root.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("lib.rs"), "pub fn answer() -> i32 { 42 }\n").unwrap();

        let att = ContextAttachment::File {
            path: "src/lib.rs".into(),
        };
        let out = expand_message("explain", std::slice::from_ref(&att), root.path()).unwrap();
        assert!(out.contains("path=\"src/lib.rs\""));
        assert!(out.contains("pub fn answer() -> i32 { 42 }"));
    }

    #[test]
    fn selection_and_terminal_and_fences_preserved() {
        let root = tempdir().unwrap();
        let atts = vec![
            ContextAttachment::Selection {
                path: "src/lib.rs".into(),
                content: "```rust\nlet x = 1;\n```".into(),
            },
            ContextAttachment::TerminalError {
                label: "rust-panic".into(),
                line: "thread 'main' panicked".into(),
            },
        ];
        let out = expand_message("why?", &atts, root.path()).unwrap();
        assert!(out.contains("<context type=\"selection\" path=\"src/lib.rs\">"));
        assert!(out.contains("```rust\nlet x = 1;\n```"));
        assert!(out.contains("<context type=\"terminal_error\" label=\"rust-panic\">"));
        assert!(out.contains("thread 'main' panicked"));
    }

    #[test]
    fn quotes_in_attributes_are_escaped() {
        let root = tempdir().unwrap();
        let att = ContextAttachment::Selection {
            path: "weird\"name.rs".into(),
            content: "x".into(),
        };
        let out = expand_message("q", std::slice::from_ref(&att), root.path()).unwrap();
        assert!(out.contains("path=\"weird&quot;name.rs\""));
    }

    #[test]
    fn binary_file_is_rejected() {
        let root = tempdir().unwrap();
        let file = root.path().join("bin.dat");
        fs::write(&file, [0xff, 0xfe, 0x00, 0x01]).unwrap();
        let att = ContextAttachment::File {
            path: file.to_string_lossy().into_owned(),
        };
        assert!(matches!(
            expand_message("x", std::slice::from_ref(&att), root.path()),
            Err(AiError::ProviderError(_))
        ));
    }

    #[test]
    fn path_outside_root_is_rejected() {
        let root = tempdir().unwrap();
        let other = tempdir().unwrap();
        let file = other.path().join("secret.txt");
        fs::write(&file, "secret").unwrap();
        let att = ContextAttachment::File {
            path: file.to_string_lossy().into_owned(),
        };
        assert!(matches!(
            expand_message("x", std::slice::from_ref(&att), root.path()),
            Err(AiError::ProviderError(_))
        ));
    }

    #[test]
    fn too_many_attachments_is_rejected() {
        let root = tempdir().unwrap();
        let atts: Vec<_> = (0..MAX_ATTACHMENTS + 1)
            .map(|i| ContextAttachment::TerminalError {
                label: "e".into(),
                line: format!("line {i}"),
            })
            .collect();
        assert!(matches!(
            expand_message("x", &atts, root.path()),
            Err(AiError::ProviderError(_))
        ));
    }

    #[test]
    fn oversized_total_is_context_length_exceeded() {
        let root = tempdir().unwrap();
        let big = "a".repeat(MAX_TOTAL_BYTES + 1);
        let att = ContextAttachment::Selection {
            path: "big.txt".into(),
            content: big,
        };
        assert!(matches!(
            expand_message("x", std::slice::from_ref(&att), root.path()),
            Err(AiError::ContextLengthExceeded)
        ));
    }
}
