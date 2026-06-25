//! Protected path registry: which paths require stronger approval or are blocked.
//!
//! The default registry is hard-coded to cover common secret and structural
//! project paths. An optional workspace override is loaded from
//! `.gwenland/safety/protected-paths.json`; if missing or malformed the
//! defaults are used. No remote policy fetch is ever made.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::agentic::policy::{is_excluded_path, is_secret_path};
use crate::safety::decision::RiskLevel;

/// How a protected path should be treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectionLevel {
    /// Ask for confirmation; allow after approval.
    Ask,
    /// Block by default; require explicit danger acknowledgment.
    Block,
}

/// One entry in the protected path registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedPathEntry {
    /// Glob-style pattern (case-insensitive, `/` and `\` both match `/`).
    /// Examples: `.env`, `.env.*`, `.git/**`, `*.pem`
    pub pattern: String,
    pub protection: ProtectionLevel,
    pub risk: RiskLevel,
    pub reason: String,
}

/// The full protected path registry (defaults + optional workspace overrides).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProtectedPathRegistry {
    pub entries: Vec<ProtectedPathEntry>,
}

impl ProtectedPathRegistry {
    /// Build the built-in default registry. Called when no workspace override
    /// file exists or when it fails to parse.
    pub fn defaults() -> Self {
        let entries = vec![
            // ---- Secret files (block) ----------------------------------------
            entry(
                ".env",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "environment file may contain secrets",
            ),
            entry(
                ".env.*",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                ".env variant may contain secrets",
            ),
            entry(
                "*.pem",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "PEM certificate or private key",
            ),
            entry(
                "*.key",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "private key file",
            ),
            entry(
                "*.pfx",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "PKCS#12 certificate bundle",
            ),
            entry(
                "*.p12",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "PKCS#12 certificate bundle",
            ),
            entry(
                "id_rsa",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "RSA private key",
            ),
            entry(
                "id_ed25519",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "Ed25519 private key",
            ),
            entry(
                "id_ecdsa",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "ECDSA private key",
            ),
            entry(
                ".ssh/**",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "SSH directory",
            ),
            entry(
                ".aws/**",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "AWS credentials directory",
            ),
            entry(
                ".gcloud/**",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "GCloud credentials directory",
            ),
            entry(
                ".gnupg/**",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "GPG key directory",
            ),
            entry(
                "secrets.*",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "secrets file",
            ),
            entry(
                "credentials.*",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "credentials file",
            ),
            entry(
                "service-account*.json",
                ProtectionLevel::Block,
                RiskLevel::Secret,
                "service account key",
            ),
            // ---- GwenLand store (ask) ----------------------------------------
            entry(
                ".gwenland/**",
                ProtectionLevel::Ask,
                RiskLevel::High,
                ".gwenland workspace store",
            ),
            // ---- VCS (ask) ---------------------------------------------------
            entry(
                ".git/**",
                ProtectionLevel::Ask,
                RiskLevel::High,
                "git repository internals",
            ),
            // ---- Lock files (ask) --------------------------------------------
            entry(
                "package-lock.json",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "npm lock file",
            ),
            entry(
                "pnpm-lock.yaml",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "pnpm lock file",
            ),
            entry(
                "yarn.lock",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "yarn lock file",
            ),
            entry(
                "Cargo.lock",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "Cargo lock file",
            ),
            entry(
                "poetry.lock",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "poetry lock file",
            ),
            entry(
                "Gemfile.lock",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "Bundler lock file",
            ),
            entry(
                "go.sum",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "Go checksum file",
            ),
            entry(
                "composer.lock",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "Composer lock file",
            ),
            // ---- Package manifests (ask) -------------------------------------
            entry(
                "package.json",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "npm package manifest",
            ),
            entry(
                "Cargo.toml",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "Cargo manifest",
            ),
            entry(
                "pyproject.toml",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "Python project config",
            ),
            entry(
                "go.mod",
                ProtectionLevel::Ask,
                RiskLevel::Medium,
                "Go module definition",
            ),
        ];
        Self { entries }
    }

    /// Load from a workspace override file (`.gwenland/safety/protected-paths.json`),
    /// falling back to `defaults()` if the file is absent or malformed.
    pub fn load(workspace_root: &Path) -> Self {
        let path = crate::workspace::safety_dir(workspace_root).join("protected-paths.json");
        if !path.exists() {
            return Self::defaults();
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) if !c.trim().is_empty() => c,
            _ => return Self::defaults(),
        };
        serde_json::from_str(&content).unwrap_or_else(|_| Self::defaults())
    }

    /// Find the first matching entry for `path_str`.
    /// Matching is case-insensitive; both `/` and `\` are treated as `/`.
    pub fn find_match(&self, path_str: &str) -> Option<&ProtectedPathEntry> {
        let norm = normalize_path(path_str);
        self.entries.iter().find(|e| {
            let pat = normalize_path(&e.pattern);
            glob_match(&pat, &norm)
        })
    }

    /// Returns the protection level + risk for `path_str`, or `None` if the
    /// path is not in the registry. Also checks the built-in secret-path
    /// heuristic from `agentic::policy` as a last resort.
    pub fn classify(&self, path_str: &str) -> Option<(&ProtectedPathEntry, bool)> {
        if let Some(entry) = self.find_match(path_str) {
            return Some((entry, entry.risk == RiskLevel::Secret));
        }
        // Fallback: the existing secret-path heuristic from agentic::policy.
        if is_secret_path(path_str) {
            // Return the first secret entry as a representative (there is always at least one).
            let representative = self.entries.iter().find(|e| e.risk == RiskLevel::Secret);
            if let Some(rep) = representative {
                return Some((rep, true));
            }
        }
        None
    }

    /// True if `path_str` is a VCS/generated/build exclusion that should never
    /// be indexed or sent to AI (re-exports `is_excluded_path`).
    pub fn is_excluded(&self, path_str: &str) -> bool {
        is_excluded_path(path_str)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn entry(
    pattern: &str,
    protection: ProtectionLevel,
    risk: RiskLevel,
    reason: &str,
) -> ProtectedPathEntry {
    ProtectedPathEntry {
        pattern: pattern.to_string(),
        protection,
        risk,
        reason: reason.to_string(),
    }
}

/// Normalize a path string: lowercase + replace `\` with `/` + strip leading `./`.
fn normalize_path(s: &str) -> String {
    s.to_ascii_lowercase()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

/// Match `path` against `pattern` (normalized, case-insensitive).
///
/// Rules:
/// - `**` matches any sequence of path components (including zero).
/// - `*` matches any characters within one path component (not `/`).
/// - A pattern with no `/` (single segment) matches against the *basename*
///   anywhere in the path, so `.env` matches both `.env` and `src/.env`, and
///   `.env.*` matches `src/.env.production`.
/// - A pattern containing `/` (multi-segment) is matched rooted from the start
///   of the normalized path, so `.git/**` matches `.git/config`.
fn glob_match(pattern: &str, path: &str) -> bool {
    let pat = normalize_path(pattern);
    let hay = normalize_path(path);

    if pat.contains('/') {
        // Multi-segment pattern: rooted segment match.
        let pat_segs: Vec<&str> = pat.split('/').collect();
        glob_segs(&pat_segs, &hay)
    } else {
        // Single-segment pattern (may contain `*`): match against every component.
        hay.split('/').any(|component| glob_seg(&pat, component))
    }
}

/// Recursive segment-level glob match. Both pattern and path are already `/`-split.
fn glob_segs(pat: &[&str], path: &str) -> bool {
    if pat.is_empty() {
        return path.is_empty();
    }

    let (head, rest) = (pat[0], &pat[1..]);

    if head == "**" {
        // `**` matches zero components, one component, or many.
        // Try matching `rest` against every suffix of `path`.
        if glob_segs(rest, path) {
            return true;
        }
        let mut remainder = path;
        loop {
            match remainder.find('/') {
                Some(i) => {
                    remainder = &remainder[i + 1..];
                    if glob_segs(rest, remainder) {
                        return true;
                    }
                }
                None => {
                    // No more slashes: advance past the last component.
                    // If rest is empty, we're done; otherwise no match.
                    return rest.is_empty() && !remainder.is_empty();
                }
            }
        }
    }

    // head is a single segment (may contain `*`).
    let (component, tail) = match path.find('/') {
        Some(i) => (&path[..i], &path[i + 1..]),
        None => {
            // No more slashes — path has one component left.
            return rest.is_empty() && glob_seg(head, path);
        }
    };

    if glob_seg(head, component) {
        glob_segs(rest, tail)
    } else {
        false
    }
}

/// Match a single path component against a glob segment (`*` allowed, no `/`).
fn glob_seg(pattern: &str, component: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == component;
    }
    // Split on `*` and require the parts appear in order in `component`.
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rem = component;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            if !rem.starts_with(part) {
                return false;
            }
            rem = &rem[part.len()..];
        } else if i == parts.len() - 1 {
            return rem.ends_with(part);
        } else {
            match rem.find(part) {
                Some(idx) => rem = &rem[idx + part.len()..],
                None => return false,
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_cover_secret_paths() {
        let reg = ProtectedPathRegistry::defaults();
        for path in [
            ".env",
            ".env.local",
            "src/.env.production",
            "server.pem",
            "private.key",
            "id_rsa",
            "id_ed25519",
            ".ssh/config",
            ".aws/credentials",
            ".gcloud/application_default_credentials.json",
            "secrets.json",
            "credentials.toml",
            "cert.pfx",
            "key.p12",
        ] {
            let hit = reg.find_match(path);
            assert!(hit.is_some(), "expected registry hit for: {path}");
            assert_eq!(
                hit.unwrap().risk,
                RiskLevel::Secret,
                "expected Secret risk for: {path}"
            );
        }
    }

    #[test]
    fn defaults_cover_gwenland_and_git() {
        let reg = ProtectedPathRegistry::defaults();
        assert!(reg.find_match(".gwenland/settings.json").is_some());
        assert!(reg.find_match(".git/config").is_some());
        assert!(reg.find_match(".git/COMMIT_EDITMSG").is_some());
    }

    #[test]
    fn defaults_cover_lock_files() {
        let reg = ProtectedPathRegistry::defaults();
        for p in [
            "package-lock.json",
            "pnpm-lock.yaml",
            "yarn.lock",
            "Cargo.lock",
            "poetry.lock",
        ] {
            assert!(reg.find_match(p).is_some(), "expected hit for lock: {p}");
        }
    }

    #[test]
    fn normal_files_not_in_registry() {
        let reg = ProtectedPathRegistry::defaults();
        for p in ["src/main.rs", "README.md", "Dockerfile", "index.html"] {
            assert!(
                reg.find_match(p).is_none(),
                "unexpected registry hit for: {p}"
            );
        }
    }

    #[test]
    fn malformed_override_file_falls_back_to_defaults() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".gwenland/safety")).unwrap();
        std::fs::write(
            dir.path().join(".gwenland/safety/protected-paths.json"),
            b"not-json",
        )
        .unwrap();
        let reg = ProtectedPathRegistry::load(dir.path());
        // Must contain at least the .env default entry.
        assert!(reg.find_match(".env").is_some());
    }

    #[test]
    fn missing_override_file_falls_back_to_defaults() {
        let dir = tempdir().unwrap();
        let reg = ProtectedPathRegistry::load(dir.path());
        assert!(reg.find_match(".env").is_some());
    }

    #[test]
    fn glob_matches_double_star() {
        assert!(glob_match(".git/**", ".git/config"));
        assert!(glob_match(".git/**", ".git/refs/heads/main"));
        assert!(glob_match(".ssh/**", ".ssh/id_rsa"));
        assert!(!glob_match(".git/**", "src/main.rs"));
    }

    #[test]
    fn glob_matches_star_extension() {
        assert!(glob_match("*.pem", "server.pem"));
        assert!(glob_match("*.pem", "cert.pem"));
        assert!(!glob_match("*.pem", "server.key"));
    }

    #[test]
    fn glob_matches_star_prefix() {
        assert!(glob_match(".env.*", ".env.local"));
        assert!(glob_match(".env.*", ".env.production"));
        assert!(!glob_match(".env.*", ".env"));
    }

    #[test]
    fn case_insensitive_matching() {
        let reg = ProtectedPathRegistry::defaults();
        assert!(reg.find_match(".ENV").is_some());
        assert!(reg.find_match("Cargo.lock").is_some());
    }
}
