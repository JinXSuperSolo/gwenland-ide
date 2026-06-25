//! Local search policy: which paths to exclude from file/text search (M14 Wave 5).
//!
//! Combines the protected-path registry's exclusion list with the existing
//! `is_secret_path` and `is_excluded_path` heuristics. No remote index is used.

use std::path::Path;

use crate::agentic::policy::{is_excluded_path, is_secret_path};
use crate::safety::protected_paths::ProtectedPathRegistry;

/// Returns `true` if `path` (relative or absolute string) should be excluded
/// from local search results: it is a secret path, a generated/dependency/VCS
/// folder, or a path the Protected Path Registry marks as blocked.
///
/// Callers pass a `workspace_root` so the registry can be loaded from
/// `.gwenland/safety/protected-paths.json` when present.
pub fn should_exclude_from_search(path: &str, workspace_root: &Path) -> bool {
    // 1. Built-in heuristics (fast, no I/O).
    if is_secret_path(path) || is_excluded_path(path) {
        return true;
    }
    // 2. Protected path registry (loads workspace override or uses defaults).
    let registry = ProtectedPathRegistry::load(workspace_root);
    if let Some((entry, is_secret)) = registry.classify(path) {
        // Block-protection or secret: exclude. Ask-only (low-risk metadata)
        // is still searchable — only block/secret paths are excluded.
        if is_secret {
            return true;
        }
        use crate::safety::protected_paths::ProtectionLevel;
        if entry.protection == ProtectionLevel::Block {
            return true;
        }
    }
    false
}

/// Quick variant that uses only the built-in heuristics (no file I/O).
/// Use this in hot loops like file-tree indexing where I/O must be minimal.
pub fn should_exclude_fast(path: &str) -> bool {
    is_secret_path(path) || is_excluded_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn secret_paths_excluded() {
        let dir = tempdir().unwrap();
        assert!(should_exclude_from_search(".env", dir.path()));
        assert!(should_exclude_from_search("server.pem", dir.path()));
        assert!(should_exclude_from_search(".ssh/id_rsa", dir.path()));
    }

    #[test]
    fn generated_dirs_excluded() {
        let dir = tempdir().unwrap();
        assert!(should_exclude_from_search(
            "node_modules/pkg/index.js",
            dir.path()
        ));
        assert!(should_exclude_from_search("target/debug/exe", dir.path()));
        assert!(should_exclude_from_search(".git/config", dir.path()));
    }

    #[test]
    fn normal_source_files_included() {
        let dir = tempdir().unwrap();
        assert!(!should_exclude_from_search("src/main.rs", dir.path()));
        assert!(!should_exclude_from_search("README.md", dir.path()));
        assert!(!should_exclude_from_search("index.html", dir.path()));
    }

    #[test]
    fn fast_path_matches_full_path_for_common_cases() {
        assert!(should_exclude_fast(".env"));
        assert!(should_exclude_fast("node_modules/foo/bar.js"));
        assert!(!should_exclude_fast("src/lib.rs"));
    }
}
