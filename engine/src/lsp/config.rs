//! LSP settings model and per-language server defaults (Milestone 6).
//!
//! The persisted shape stores command/args overrides only — never secrets, API
//! keys, or environment dumps (Requirement 4.4). All fields use
//! `#[serde(default)]` so M1–M5 settings files (which have no `[lsp]` table)
//! still load cleanly (Requirement 4.2/4.5).
//!
//! Default commands (Requirements 3.3–3.6):
//! - Rust:                `rust-analyzer`
//! - TypeScript/JavaScript: `typescript-language-server --stdio`
//! - Python:              `pyright-langserver --stdio`
//! - Python fallback:     `pylsp` — selected by overriding the command in
//!   settings (e.g. `command = "pylsp"`, `args = []`).

use serde::{Deserialize, Serialize};

use super::language::LanguageId;

fn default_true() -> bool {
    true
}

/// Per-language server configuration. Empty `command` means "use the built-in
/// default for this language"; see [`LanguageServerSettings::effective`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguageServerSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl Default for LanguageServerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            command: String::new(),
            args: Vec::new(),
        }
    }
}

impl LanguageServerSettings {
    /// Resolve the effective `(command, args)` for a server bucket, applying the
    /// built-in default when `command` is empty.
    ///
    /// Override semantics: if the user sets a non-empty `command`, their `args`
    /// are used verbatim (possibly empty — correct for `pylsp`, which speaks
    /// stdio with no flag). Only when the command is left at the default do we
    /// also substitute the default args.
    pub fn effective(&self, server_key: &str) -> (String, Vec<String>) {
        let (default_cmd, default_args) = default_command(server_key);
        if self.command.trim().is_empty() {
            (default_cmd.to_string(), default_args)
        } else {
            (self.command.clone(), self.args.clone())
        }
    }
}

/// The `[lsp]` section of `Settings`. One bucket per server. TypeScript and
/// JavaScript share the `typescript` bucket (Requirement 2.7).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LspSettings {
    #[serde(default)]
    pub rust: LanguageServerSettings,
    #[serde(default)]
    pub typescript: LanguageServerSettings,
    #[serde(default)]
    pub python: LanguageServerSettings,
}

impl LspSettings {
    /// The settings bucket for a language's server (TS/JS → typescript).
    pub fn for_language(&self, lang: LanguageId) -> &LanguageServerSettings {
        match lang.server_key() {
            "rust" => &self.rust,
            "typescript" => &self.typescript,
            "python" => &self.python,
            _ => &self.rust, // unreachable: server_key() only yields the three
        }
    }
}

/// The built-in default `(command, args)` for a server bucket key
/// (`"rust"`, `"typescript"`, `"python"`).
pub fn default_command(server_key: &str) -> (&'static str, Vec<String>) {
    match server_key {
        "rust" => ("rust-analyzer", Vec::new()),
        "typescript" => ("typescript-language-server", vec!["--stdio".to_string()]),
        "python" => ("pyright-langserver", vec!["--stdio".to_string()]),
        _ => ("", Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_enabled_with_empty_overrides() {
        let s = LspSettings::default();
        assert!(s.rust.enabled);
        assert!(s.typescript.enabled);
        assert!(s.python.enabled);
        assert!(s.rust.command.is_empty());
        assert!(s.typescript.args.is_empty());
    }

    #[test]
    fn default_commands_match_design() {
        assert_eq!(default_command("rust"), ("rust-analyzer", vec![]));
        assert_eq!(
            default_command("typescript"),
            ("typescript-language-server", vec!["--stdio".to_string()])
        );
        assert_eq!(
            default_command("python"),
            ("pyright-langserver", vec!["--stdio".to_string()])
        );
    }

    #[test]
    fn effective_uses_defaults_when_command_blank() {
        let s = LanguageServerSettings::default();
        assert_eq!(
            s.effective("typescript"),
            (
                "typescript-language-server".to_string(),
                vec!["--stdio".to_string()]
            )
        );
    }

    #[test]
    fn effective_uses_override_command_and_args() {
        // Python fallback to pylsp: explicit command, no args.
        let s = LanguageServerSettings {
            enabled: true,
            command: "pylsp".to_string(),
            args: vec![],
        };
        assert_eq!(s.effective("python"), ("pylsp".to_string(), vec![]));

        // Custom absolute path with explicit args.
        let s = LanguageServerSettings {
            enabled: true,
            command: "/opt/ra/rust-analyzer".to_string(),
            args: vec!["--log-file".to_string(), "/tmp/ra.log".to_string()],
        };
        assert_eq!(
            s.effective("rust"),
            (
                "/opt/ra/rust-analyzer".to_string(),
                vec!["--log-file".to_string(), "/tmp/ra.log".to_string()]
            )
        );
    }

    #[test]
    fn for_language_maps_ts_and_js_to_typescript_bucket() {
        let mut s = LspSettings::default();
        s.typescript.command = "tsserver-custom".to_string();
        assert_eq!(
            s.for_language(LanguageId::TypeScript).command,
            "tsserver-custom"
        );
        assert_eq!(
            s.for_language(LanguageId::JavaScript).command,
            "tsserver-custom"
        );
        assert_eq!(s.for_language(LanguageId::Rust).command, "");
    }

    #[test]
    fn lsp_settings_round_trip_through_toml() {
        let mut s = LspSettings::default();
        s.python.command = "pylsp".to_string();
        s.python.enabled = false;
        let toml = toml::to_string(&s).unwrap();
        let back: LspSettings = toml::from_str(&toml).unwrap();
        assert_eq!(s, back);
    }
}
