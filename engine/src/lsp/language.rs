//! Language identification and file-extension mapping (Milestone 6).
//!
//! M6 supports exactly four LSP language ids — Rust, TypeScript, JavaScript,
//! and Python — driven entirely by file extension. TypeScript and JavaScript
//! intentionally map to *distinct* [`LanguageId`]s (so the correct `languageId`
//! string is sent in `didOpen`) but share one server process per workspace;
//! that sharing is handled by [`LanguageId::server_key`], not here.

use serde::{Deserialize, Serialize};

/// One of the four languages M6 understands. Anything else leaves the file in
/// plain-editor mode (Requirement 2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageId {
    Rust,
    TypeScript,
    JavaScript,
    Python,
}

impl LanguageId {
    /// The LSP `languageId` string sent in `textDocument/didOpen`
    /// (Requirements 2.1–2.4).
    pub fn as_lsp_language_id(self) -> &'static str {
        match self {
            LanguageId::Rust => "rust",
            LanguageId::TypeScript => "typescript",
            LanguageId::JavaScript => "javascript",
            LanguageId::Python => "python",
        }
    }

    /// The settings/server bucket this language uses. TypeScript and JavaScript
    /// collapse to the same `typescript` server (Requirement 2.7); Rust and
    /// Python each have their own.
    pub fn server_key(self) -> &'static str {
        match self {
            LanguageId::Rust => "rust",
            LanguageId::TypeScript | LanguageId::JavaScript => "typescript",
            LanguageId::Python => "python",
        }
    }

    /// Parse a settings/server bucket key back into a representative
    /// [`LanguageId`]. `"typescript"` resolves to [`LanguageId::TypeScript`].
    /// Used by `lsp_restart`, which is keyed by language string.
    pub fn from_server_key(key: &str) -> Option<LanguageId> {
        match key {
            "rust" => Some(LanguageId::Rust),
            "typescript" | "javascript" => Some(LanguageId::TypeScript),
            "python" => Some(LanguageId::Python),
            _ => None,
        }
    }

    /// Map a file extension (without the leading dot, case-insensitive) to its
    /// language. Returns `None` for unsupported extensions (Requirement 2.5/2.6).
    pub fn from_extension(ext: &str) -> Option<LanguageId> {
        match ext.to_ascii_lowercase().as_str() {
            "rs" => Some(LanguageId::Rust),
            "ts" | "tsx" => Some(LanguageId::TypeScript),
            "js" | "jsx" | "mjs" | "cjs" => Some(LanguageId::JavaScript),
            "py" => Some(LanguageId::Python),
            _ => None,
        }
    }

    /// Map a file path to its language by extension. Convenience over
    /// [`LanguageId::from_extension`].
    pub fn from_path(path: &std::path::Path) -> Option<LanguageId> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(LanguageId::from_extension)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn extension_mapping_covers_all_required_extensions() {
        assert_eq!(LanguageId::from_extension("rs"), Some(LanguageId::Rust));
        assert_eq!(
            LanguageId::from_extension("ts"),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(
            LanguageId::from_extension("tsx"),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(
            LanguageId::from_extension("js"),
            Some(LanguageId::JavaScript)
        );
        assert_eq!(
            LanguageId::from_extension("jsx"),
            Some(LanguageId::JavaScript)
        );
        assert_eq!(
            LanguageId::from_extension("mjs"),
            Some(LanguageId::JavaScript)
        );
        assert_eq!(
            LanguageId::from_extension("cjs"),
            Some(LanguageId::JavaScript)
        );
        assert_eq!(LanguageId::from_extension("py"), Some(LanguageId::Python));
    }

    #[test]
    fn extension_mapping_is_case_insensitive() {
        assert_eq!(LanguageId::from_extension("RS"), Some(LanguageId::Rust));
        assert_eq!(
            LanguageId::from_extension("Ts"),
            Some(LanguageId::TypeScript)
        );
    }

    #[test]
    fn unsupported_extensions_return_none() {
        for ext in ["txt", "md", "json", "toml", "c", "cpp", "go", ""] {
            assert_eq!(LanguageId::from_extension(ext), None, "ext={ext}");
        }
    }

    #[test]
    fn from_path_uses_extension() {
        assert_eq!(
            LanguageId::from_path(Path::new("/home/user/main.rs")),
            Some(LanguageId::Rust)
        );
        assert_eq!(
            LanguageId::from_path(Path::new("C:\\proj\\app.tsx")),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(LanguageId::from_path(Path::new("README")), None);
    }

    #[test]
    fn language_id_strings() {
        assert_eq!(LanguageId::Rust.as_lsp_language_id(), "rust");
        assert_eq!(LanguageId::TypeScript.as_lsp_language_id(), "typescript");
        assert_eq!(LanguageId::JavaScript.as_lsp_language_id(), "javascript");
        assert_eq!(LanguageId::Python.as_lsp_language_id(), "python");
    }

    #[test]
    fn ts_and_js_share_a_server_key() {
        assert_eq!(LanguageId::TypeScript.server_key(), "typescript");
        assert_eq!(LanguageId::JavaScript.server_key(), "typescript");
        assert_eq!(LanguageId::Rust.server_key(), "rust");
        assert_eq!(LanguageId::Python.server_key(), "python");
    }

    #[test]
    fn from_server_key_round_trips() {
        assert_eq!(LanguageId::from_server_key("rust"), Some(LanguageId::Rust));
        assert_eq!(
            LanguageId::from_server_key("typescript"),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(
            LanguageId::from_server_key("python"),
            Some(LanguageId::Python)
        );
        assert_eq!(LanguageId::from_server_key("ruby"), None);
    }
}
