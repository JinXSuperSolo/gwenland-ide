//! Workspace root detection and `file://` URI helpers (Milestone 6).
//!
//! Root detection walks from the file's directory upward, stopping at the open
//! workspace root, returning the nearest ancestor that holds a language-specific
//! marker file (Requirement 5.1–5.8). M6 assumes a single root per client.
//!
//! URI helpers are hand-rolled (no `url` crate) to keep the engine's dependency
//! footprint — and the release binary — small. They cover the path shapes M6
//! produces: Windows drive paths and POSIX absolute paths.

use std::path::{Path, PathBuf};

use super::language::LanguageId;

/// Marker files searched per language, in no particular priority within a
/// directory (presence of any one is enough). Order across directories is
/// nearest-first via the ancestor walk.
fn markers_for(lang: LanguageId) -> &'static [&'static str] {
    match lang {
        LanguageId::Rust => &["Cargo.toml"],
        LanguageId::TypeScript | LanguageId::JavaScript => {
            &["tsconfig.json", "jsconfig.json", "package.json"]
        }
        LanguageId::Python => &[
            "pyproject.toml",
            "setup.py",
            "setup.cfg",
            "requirements.txt",
        ],
    }
}

fn dir_has_marker(dir: &Path, lang: LanguageId) -> bool {
    markers_for(lang).iter().any(|m| dir.join(m).is_file())
}

/// Detect the workspace root to advertise to the language server.
///
/// 1. Start at the file's parent directory.
/// 2. Walk ancestors upward (stopping at `workspace_root` when given).
/// 3. Return the first ancestor containing a marker for `lang`.
/// 4. Otherwise return `workspace_root` if provided.
/// 5. Otherwise return the file's parent directory.
/// Normalize a path for case-insensitive and separator-insensitive systems (like Windows).
/// On Windows, this:
/// 1. Converts backslashes to forward slashes.
/// 2. Lowercases the drive letter (e.g. "C:/" -> "c:/").
/// On POSIX, it just returns the path as-is (with forward slashes).
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut s = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows)
        && s.len() >= 2 && s.as_bytes()[1] == b':' && s.as_bytes()[0].is_ascii_alphabetic() {
            let drive = s.as_bytes()[0].to_ascii_lowercase() as char;
            s = format!("{}{}", drive, &s[1..]);
        }
    PathBuf::from(s)
}

pub fn detect_root(file_path: &Path, lang: LanguageId, workspace_root: Option<&Path>) -> PathBuf {
    let file_path = normalize_path(file_path);
    let workspace_root = workspace_root.map(normalize_path);
    let start = file_path.parent().unwrap_or(&file_path);

    for dir in start.ancestors() {
        if dir_has_marker(dir, lang) {
            return dir.to_path_buf();
        }
        // Do not climb above the open workspace root.
        if let Some(ref ws) = workspace_root
            && dir == ws
        {
            break;
        }
    }

    if let Some(ws) = workspace_root {
        return ws.to_path_buf();
    }
    start.to_path_buf()
}

/// Percent-encode a forward-slash path string, leaving the path-safe set —
/// unreserved chars plus `/` and `:` (drive colon) — intact. Operates on UTF-8
/// bytes so non-ASCII is encoded correctly.
fn percent_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Convert an absolute filesystem path into a `file://` URI for LSP payloads
/// (Requirement 5.7). Backslashes are normalized to forward slashes.
pub fn path_to_file_uri(path: &Path) -> String {
    let normalized_path = normalize_path(path);
    let normalized = normalized_path.to_string_lossy().to_string();
    let encoded = percent_encode_path(&normalized);
    if encoded.starts_with('/') {
        // POSIX: "/home/x" -> "file:///home/x"
        format!("file://{encoded}")
    } else {
        // Windows: "c:/Users/x" -> "file:///c:/Users/x"
        format!("file:///{encoded}")
    }
}

/// Convert a `file://` URI back into a filesystem path where the shape is
/// understood (Requirement 14.3). Returns `None` for non-`file:` URIs.
pub fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let decoded = percent_decode(rest);

    // A Windows drive URI decodes to "/C:/Users/x"; drop the leading slash.
    let trimmed = decoded.strip_prefix('/').unwrap_or(&decoded);
    let looks_like_drive = trimmed.len() >= 2
        && trimmed.as_bytes()[1] == b':'
        && trimmed.as_bytes()[0].is_ascii_alphabetic();

    let path = if looks_like_drive {
        PathBuf::from(trimmed)
    } else {
        PathBuf::from(decoded)
    };
    Some(normalize_path(&path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), b"").unwrap();
    }

    #[test]
    fn detects_rust_root_via_cargo_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(root, "Cargo.toml");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        let file = src.join("main.rs");

        let detected = detect_root(&file, LanguageId::Rust, Some(root));
        assert_eq!(detected, root);
    }

    #[test]
    fn detects_ts_root_via_package_json() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(root, "package.json");
        let nested = root.join("src/components");
        fs::create_dir_all(&nested).unwrap();
        let file = nested.join("App.tsx");

        let detected = detect_root(&file, LanguageId::TypeScript, Some(root));
        assert_eq!(detected, root);
    }

    #[test]
    fn js_uses_same_markers_as_ts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(root, "jsconfig.json");
        let file = root.join("index.js");
        let detected = detect_root(&file, LanguageId::JavaScript, Some(root));
        assert_eq!(detected, root);
    }

    #[test]
    fn detects_python_root_via_pyproject() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(root, "pyproject.toml");
        let pkg = root.join("pkg");
        fs::create_dir_all(&pkg).unwrap();
        let file = pkg.join("mod.py");

        let detected = detect_root(&file, LanguageId::Python, Some(root));
        assert_eq!(detected, root);
    }

    #[test]
    fn picks_nearest_marker_not_outermost() {
        let tmp = tempfile::tempdir().unwrap();
        let outer = tmp.path();
        touch(outer, "Cargo.toml");
        let inner = outer.join("crates/sub");
        fs::create_dir_all(&inner).unwrap();
        touch(&inner, "Cargo.toml");
        let file = inner.join("lib.rs");

        let detected = detect_root(&file, LanguageId::Rust, Some(outer));
        assert_eq!(detected, inner);
    }

    #[test]
    fn falls_back_to_workspace_root_when_no_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let sub = root.join("a/b");
        fs::create_dir_all(&sub).unwrap();
        let file = sub.join("main.rs");

        let detected = detect_root(&file, LanguageId::Rust, Some(root));
        assert_eq!(detected, root);
    }

    #[test]
    fn falls_back_to_file_parent_when_no_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("loose");
        fs::create_dir_all(&sub).unwrap();
        let file = sub.join("script.py");

        let detected = detect_root(&file, LanguageId::Python, None);
        assert_eq!(detected, sub);
    }

    #[test]
    fn does_not_climb_above_workspace_root() {
        let tmp = tempfile::tempdir().unwrap();
        // Marker exists ABOVE the workspace root; must not be selected.
        touch(tmp.path(), "Cargo.toml");
        let ws = tmp.path().join("workspace");
        let src = ws.join("src");
        fs::create_dir_all(&src).unwrap();
        let file = src.join("main.rs");

        let detected = detect_root(&file, LanguageId::Rust, Some(&ws));
        assert_eq!(detected, ws);
    }

    #[test]
    fn posix_path_to_uri() {
        let uri = path_to_file_uri(Path::new("/home/user/main.rs"));
        assert_eq!(uri, "file:///home/user/main.rs");
    }

    #[test]
    fn posix_path_with_spaces_is_encoded() {
        let uri = path_to_file_uri(Path::new("/home/my user/a b.rs"));
        assert_eq!(uri, "file:///home/my%20user/a%20b.rs");
    }

    #[test]
    fn windows_path_to_uri() {
        let uri = path_to_file_uri(Path::new("C:\\Users\\foo\\main.rs"));
        assert_eq!(uri, "file:///c:/Users/foo/main.rs");
    }

    #[test]
    fn uri_round_trips_posix() {
        let p = Path::new("/home/user/a b.rs");
        let uri = path_to_file_uri(p);
        let back = file_uri_to_path(&uri).unwrap();
        assert_eq!(back, PathBuf::from("/home/user/a b.rs"));
    }

    #[test]
    fn uri_round_trips_windows_drive() {
        let uri = "file:///C:/Users/foo/main.rs";
        let back = file_uri_to_path(uri).unwrap();
        assert_eq!(back, PathBuf::from("c:/Users/foo/main.rs"));
    }

    #[test]
    fn non_file_uri_returns_none() {
        assert_eq!(file_uri_to_path("http://example.com"), None);
    }
}
