//! Safety policy: path canonicalization, secret/exclusion denylists, command
//! risk classification, and inline-secret redaction (M10, Requirements 3 & 7).
//!
//! Everything here is pure (no Tauri/UI). The command layer calls these helpers
//! before reading context, before applying edits, and before approving a
//! validation command. The bias is conservative: when in doubt, exclude or block.

use std::path::{Component, Path, PathBuf};

use crate::agentic::validation::CommandRisk;

/// Why a path/command was refused by policy. User-safe; never contains secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// The path resolved outside the workspace root.
    OutsideWorkspace,
    /// The workspace root or path could not be canonicalized.
    Unresolvable(String),
    /// The path matched a secret denylist pattern.
    SecretPath,
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::OutsideWorkspace => write!(f, "path is outside the workspace root"),
            PolicyError::Unresolvable(p) => write!(f, "cannot resolve path: {p}"),
            PolicyError::SecretPath => write!(f, "path matches a secret pattern"),
        }
    }
}

impl std::error::Error for PolicyError {}

// --- Path helpers ----------------------------------------------------------

/// Split a path string into lowercased, separator-normalized components. Handles
/// both `/` and `\` so policy is identical across OSes and on model-produced
/// paths (which usually use `/`).
fn components_lower(path: &str) -> Vec<String> {
    path.split(['/', '\\'])
        .filter(|c| !c.is_empty() && *c != ".")
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// True if `path` matches a common secret pattern and must be excluded from
/// automatic context by default (Requirement 3.3). Matching is name/extension
/// based and case-insensitive.
///
/// Patterns: `.env`, `.env.*`, `*.pem`, `*.key`, `id_rsa`, `id_ed25519`,
/// `.ssh/**`, `.aws/**`, `.gcloud/**`, `secrets.*`, `credentials.*`.
pub fn is_secret_path(path: &str) -> bool {
    let comps = components_lower(path);
    // Any sensitive directory anywhere in the path.
    const SECRET_DIRS: &[&str] = &[".ssh", ".aws", ".gcloud", ".gnupg"];
    if comps.iter().any(|c| SECRET_DIRS.contains(&c.as_str())) {
        return true;
    }
    let name = match comps.last() {
        Some(n) => n.as_str(),
        None => return false,
    };
    // Exact / prefix filename matches.
    if name == ".env" || name.starts_with(".env.") || name == "id_rsa" || name == "id_ed25519" {
        return true;
    }
    if name.starts_with("secrets.") || name.starts_with("credentials.") {
        return true;
    }
    // Extension matches.
    if name.ends_with(".pem")
        || name.ends_with(".key")
        || name.ends_with(".pfx")
        || name.ends_with(".p12")
    {
        return true;
    }
    false
}

/// True if `path` is in a generated/dependency/build/VCS folder that should be
/// excluded from automatic context (Requirement 3.5).
pub fn is_excluded_path(path: &str) -> bool {
    const EXCLUDED_DIRS: &[&str] = &[
        ".git",
        "node_modules",
        "dist",
        "build",
        "target",
        ".svelte-kit",
        ".next",
        ".nuxt",
        "out",
        "coverage",
        ".turbo",
        "vendor",
    ];
    components_lower(path)
        .iter()
        .any(|c| EXCLUDED_DIRS.contains(&c.as_str()))
}

/// Canonicalize `path` and ensure it stays under `workspace_root`. Tolerates a
/// not-yet-existing target (for create-file edits) by resolving the deepest
/// existing ancestor, mirroring `crate::fs`. Returns the canonical path on
/// success.
pub fn canonical_within_workspace(
    path: &Path,
    workspace_root: &Path,
) -> Result<PathBuf, PolicyError> {
    let root = workspace_root
        .canonicalize()
        .map_err(|_| PolicyError::Unresolvable(workspace_root.to_string_lossy().into_owned()))?;
    let target = resolve_existing_ancestor(path)?;
    // The root itself is not a valid edit target; it must be a child.
    if target != root && target.starts_with(&root) {
        Ok(target)
    } else {
        Err(PolicyError::OutsideWorkspace)
    }
}

/// True if `path` resolves inside `workspace_root` (never panics).
pub fn is_within_workspace(path: &Path, workspace_root: &Path) -> bool {
    canonical_within_workspace(path, workspace_root).is_ok()
}

/// Resolve `path` to an absolute, symlink-free path even when it does not yet
/// exist, by canonicalizing the deepest existing ancestor and re-appending the
/// remaining components (rejecting any `..` that escapes).
fn resolve_existing_ancestor(path: &Path) -> Result<PathBuf, PolicyError> {
    if path.exists() {
        return path
            .canonicalize()
            .map_err(|_| PolicyError::Unresolvable(path.to_string_lossy().into_owned()));
    }
    let mut base = path;
    let mut tail_owned: Vec<Component> = Vec::new();
    loop {
        match base.parent() {
            Some(parent) if parent.exists() => {
                if let Some(name) = base.file_name() {
                    tail_owned.push(Component::Normal(name));
                }
                base = parent;
                break;
            }
            Some(parent) => {
                if let Some(name) = base.file_name() {
                    tail_owned.push(Component::Normal(name));
                }
                base = parent;
            }
            None => return Err(PolicyError::OutsideWorkspace),
        }
    }
    let mut resolved = base
        .canonicalize()
        .map_err(|_| PolicyError::Unresolvable(base.to_string_lossy().into_owned()))?;
    for component in tail_owned.into_iter().rev() {
        match component {
            Component::Normal(c) => resolved.push(c),
            Component::ParentDir => {
                if !resolved.pop() {
                    return Err(PolicyError::OutsideWorkspace);
                }
            }
            Component::RootDir | Component::Prefix(_) => return Err(PolicyError::OutsideWorkspace),
            Component::CurDir => {}
        }
    }
    Ok(resolved)
}

// --- Command risk classification -------------------------------------------

/// Classify a shell command's risk (Requirement 7.4). Conservative: anything
/// not confidently recognized is [`CommandRisk::Blocked`]. Destructive markers
/// win over everything so a chained command like `build && rm -rf x` is
/// classified destructive.
pub fn classify_command(command: &str) -> CommandRisk {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return CommandRisk::Blocked;
    }
    let lower = trimmed.to_ascii_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();

    if is_destructive(&lower, &tokens) {
        return CommandRisk::Destructive;
    }
    if is_dependency_changing(&tokens) {
        return CommandRisk::DependencyChanging;
    }
    if is_file_mutating(&lower) {
        return CommandRisk::FileMutating;
    }
    if is_safe_check(&lower) {
        return CommandRisk::SafeCheck;
    }
    CommandRisk::Blocked
}

fn is_destructive(lower: &str, tokens: &[&str]) -> bool {
    const DESTRUCTIVE_TOKENS: &[&str] = &[
        "rm", "rmdir", "del", "erase", "shred", "truncate", "dd", "mkfs", "fdisk", "format",
    ];
    if tokens.iter().any(|t| DESTRUCTIVE_TOKENS.contains(t)) {
        return true;
    }
    const DESTRUCTIVE_PATTERNS: &[&str] = &[
        "-rf",
        "-fr",
        "--force",
        "--hard",
        "git reset",
        "git clean",
        "reset --hard",
        "checkout --",
        "push --force",
        "push -f",
        "push origin +",
        "cargo clean",
        "drop database",
        "drop table",
        "rd /s",
        "rmdir /s",
        "> /dev/sd",
    ];
    DESTRUCTIVE_PATTERNS.iter().any(|p| lower.contains(p))
}

fn is_dependency_changing(tokens: &[&str]) -> bool {
    const MANAGERS: &[&str] = &[
        "npm", "pnpm", "yarn", "cargo", "pip", "pip3", "poetry", "bun", "go", "gem", "composer",
        "apt", "apt-get", "brew", "dotnet", "nuget",
    ];
    const DEP_SUBCMDS: &[&str] = &[
        "install",
        "add",
        "remove",
        "uninstall",
        "update",
        "upgrade",
        "ci",
    ];
    let Some(first) = tokens.first() else {
        return false;
    };
    if !MANAGERS.contains(first) {
        return false;
    }
    // `npm i`, `go get`, plus the explicit subcommands above.
    tokens.iter().skip(1).any(|t| {
        DEP_SUBCMDS.contains(t) || (*first == "npm" && *t == "i") || (*first == "go" && *t == "get")
    })
}

fn is_file_mutating(lower: &str) -> bool {
    // Format/lint in check/dry-run mode is a safe check, not a mutation.
    let check_mode = lower.contains("--check")
        || lower.contains("--dry-run")
        || lower.contains("--list-different");
    let explicit_write = lower.contains("--write") || lower.contains("--fix");
    if check_mode && !explicit_write {
        return false;
    }
    if explicit_write {
        return true;
    }
    const MUTATING_PATTERNS: &[&str] = &[
        "prettier",
        "rustfmt",
        "cargo fmt",
        "npm run format",
        "pnpm format",
        "yarn format",
        "migrate",
        "migration",
        "codegen",
        "sed -i",
        "black ",
        "gofmt -w",
    ];
    MUTATING_PATTERNS.iter().any(|p| lower.contains(p))
}

fn is_safe_check(lower: &str) -> bool {
    const SAFE_PATTERNS: &[&str] = &[
        "check",
        "test",
        "build",
        "lint",
        "clippy",
        "tsc",
        "typecheck",
        "type-check",
        "eslint",
        "--check",
        "--dry-run",
        "jest",
        "vitest",
        "pytest",
        "mocha",
        "go vet",
        "svelte-check",
        "audit",
        "doc",
    ];
    SAFE_PATTERNS.iter().any(|p| lower.contains(p))
}

// --- Inline secret redaction -----------------------------------------------

/// Redact secret-looking inline values from `text` (provider keys, GitHub
/// tokens, private-key blocks, long bearer tokens). Returns the scrubbed text
/// and whether anything was redacted. Best-effort defense-in-depth on top of the
/// path denylist (Requirement 3 / design "Safety Policy").
pub fn redact_secrets(text: &str) -> (String, bool) {
    let mut out = String::with_capacity(text.len());
    let mut redacted = false;

    // Private key PEM blocks: redact the whole block from BEGIN to the closing
    // dashes of the END line.
    let mut rest = text;
    while let Some(begin) = rest.find("-----BEGIN ") {
        let region = &rest[begin..];
        // Confirm this is a private-key header and has a matching END marker.
        if region.contains("PRIVATE KEY-----")
            && let Some(end_rel) = region.find("-----END ")
        {
            let after_end_marker = begin + end_rel + "-----END ".len();
            if let Some(close_rel) = rest[after_end_marker..].find("-----") {
                let block_end = after_end_marker + close_rel + "-----".len();
                out.push_str(&rest[..begin]);
                out.push_str("[REDACTED PRIVATE KEY]");
                redacted = true;
                rest = &rest[block_end..];
                continue;
            }
        }
        // Not a redactable key block — emit through the BEGIN marker and keep
        // scanning (so we never loop forever on the same match).
        let advance = begin + "-----BEGIN ".len();
        out.push_str(&rest[..advance]);
        rest = &rest[advance..];
    }
    out.push_str(rest);

    // Token-shaped substrings, scrubbed word by word.
    let scrubbed: String = out
        .split_inclusive(|c: char| c.is_whitespace())
        .map(|chunk| {
            let trimmed = chunk.trim_end();
            if looks_like_token(trimmed) {
                redacted = true;
                let ws = &chunk[trimmed.len()..];
                format!("[REDACTED]{ws}")
            } else {
                chunk.to_string()
            }
        })
        .collect();

    (scrubbed, redacted)
}

/// Heuristic: does this bare word look like a credential?
fn looks_like_token(word: &str) -> bool {
    let w = word.trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == ';');
    if w.len() < 16 {
        return false;
    }
    // Known provider/token prefixes.
    const PREFIXES: &[&str] = &[
        "sk-",
        "sk-ant-",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "AKIA",
        "ASIA",
        "AIza",
        "ya29.",
        "glpat-",
    ];
    if PREFIXES.iter().any(|p| w.starts_with(p)) {
        return true;
    }
    // Long high-entropy-ish bearer-style tokens (letters+digits, length >= 32).
    if w.len() >= 32 {
        let alnum = w.chars().filter(|c| c.is_ascii_alphanumeric()).count();
        let has_digit = w.chars().any(|c| c.is_ascii_digit());
        let has_alpha = w.chars().any(|c| c.is_ascii_alphabetic());
        if alnum >= w.len().saturating_sub(4) && has_digit && has_alpha {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn secret_paths_are_detected() {
        for p in [
            ".env",
            ".env.local",
            "config/.env.production",
            "keys/server.pem",
            "deploy/private.key",
            "home/.ssh/id_rsa",
            "id_ed25519",
            "app/secrets.toml",
            "credentials.json",
            "project/.aws/config",
            "certs/cert.pfx",
        ] {
            assert!(is_secret_path(p), "expected secret: {p}");
        }
    }

    #[test]
    fn normal_paths_are_not_secret() {
        for p in [
            "src/main.rs",
            "README.md",
            "environment.ts",
            "src/keyboard.ts",
            "package.json",
        ] {
            assert!(!is_secret_path(p), "false positive secret: {p}");
        }
    }

    #[test]
    fn excluded_dirs_are_detected() {
        assert!(is_excluded_path("node_modules/react/index.js"));
        assert!(is_excluded_path("target/debug/foo"));
        assert!(is_excluded_path(".git/config"));
        assert!(is_excluded_path("frontend/ui/dist/app.js"));
        assert!(!is_excluded_path("src/lib/main.rs"));
    }

    #[test]
    fn workspace_traversal_is_rejected() {
        let ws = tempdir().unwrap();
        let inside = ws.path().join("src/main.rs");
        fs::create_dir_all(inside.parent().unwrap()).unwrap();
        fs::write(&inside, "x").unwrap();
        assert!(is_within_workspace(&inside, ws.path()));

        // `..` escape and a sibling-outside path are rejected.
        let escape = ws.path().join("../escape.txt");
        assert!(matches!(
            canonical_within_workspace(&escape, ws.path()),
            Err(PolicyError::OutsideWorkspace)
        ));

        let outside = tempdir().unwrap();
        let outfile = outside.path().join("x.txt");
        fs::write(&outfile, "x").unwrap();
        assert!(!is_within_workspace(&outfile, ws.path()));
    }

    #[test]
    fn new_file_under_workspace_resolves() {
        let ws = tempdir().unwrap();
        let new_file = ws.path().join("src/new/module.rs");
        // Parent does not exist yet; still must resolve as inside.
        assert!(is_within_workspace(&new_file, ws.path()));
    }

    #[test]
    fn destructive_commands_classified_conservatively() {
        for c in [
            "rm -rf node_modules",
            "git reset --hard HEAD~1",
            "git clean -fd",
            "cargo clean",
            "git push --force origin main",
            "del /f /q file.txt",
            "git checkout -- src/main.rs",
            "cargo build && rm -rf dist",
        ] {
            assert_eq!(classify_command(c), CommandRisk::Destructive, "cmd: {c}");
        }
    }

    #[test]
    fn dependency_commands_classified() {
        for c in [
            "pnpm install",
            "npm i",
            "cargo add serde",
            "yarn add react",
            "pip install requests",
            "go get ./...",
            "npm uninstall foo",
        ] {
            assert_eq!(
                classify_command(c),
                CommandRisk::DependencyChanging,
                "cmd: {c}"
            );
        }
    }

    #[test]
    fn safe_checks_classified() {
        for c in [
            "cargo check --workspace",
            "cargo test",
            "pnpm build",
            "pnpm check",
            "pnpm test",
            "tsc --noEmit",
            "eslint .",
            "prettier --check .",
        ] {
            assert_eq!(classify_command(c), CommandRisk::SafeCheck, "cmd: {c}");
        }
    }

    #[test]
    fn file_mutating_classified() {
        for c in [
            "prettier --write .",
            "cargo fmt",
            "eslint --fix src",
            "rustfmt src/main.rs",
        ] {
            assert_eq!(classify_command(c), CommandRisk::FileMutating, "cmd: {c}");
        }
    }

    #[test]
    fn unknown_and_empty_commands_are_blocked() {
        assert_eq!(classify_command(""), CommandRisk::Blocked);
        assert_eq!(classify_command("   "), CommandRisk::Blocked);
        assert_eq!(classify_command("frobnicate --wat"), CommandRisk::Blocked);
    }

    #[test]
    fn redacts_provider_keys_and_tokens() {
        let (out, hit) = redact_secrets("export KEY=sk-ant-abcdefghijklmnopqrstuvwxyz0123456789");
        assert!(hit);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("sk-ant-abcdefghij"));

        let (out2, hit2) = redact_secrets("token ghp_0123456789abcdefghijABCDEFGHIJ0123 ok");
        assert!(hit2);
        assert!(out2.contains("[REDACTED]"));
        assert!(out2.contains("ok"));
    }

    #[test]
    fn redacts_private_key_block() {
        let pem =
            "before\n-----BEGIN PRIVATE KEY-----\nMIIBVgIBADANBg\n-----END PRIVATE KEY-----\nafter";
        let (out, hit) = redact_secrets(pem);
        assert!(hit);
        assert!(out.contains("[REDACTED PRIVATE KEY]"));
        assert!(out.contains("before"));
        assert!(out.contains("after"));
        assert!(!out.contains("MIIBVgIBADANBg"));
    }

    #[test]
    fn leaves_ordinary_text_untouched() {
        let (out, hit) = redact_secrets("the quick brown fox jumps over the lazy dog");
        assert!(!hit);
        assert_eq!(out, "the quick brown fox jumps over the lazy dog");
    }
}
