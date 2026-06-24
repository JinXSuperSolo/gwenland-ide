use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
pub enum FsError {
    #[error("I/O error: {0}")]
    Io(String),
    #[error("path contains invalid UTF-8")]
    InvalidUtf8,
    #[error("folder selection was cancelled")]
    DialogCancelled,
    #[error("not a directory: {0}")]
    NotADirectory(String),
    #[error("binary file cannot be opened as text")]
    BinaryFile,
    #[error("path is outside the workspace")]
    OutsideWorkspace,
    #[error("path already exists: {0}")]
    AlreadyExists(String),
    #[error("path does not exist: {0}")]
    NotFound(String),
}

impl From<std::io::Error> for FsError {
    fn from(err: std::io::Error) -> Self {
        FsError::Io(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

/// Lists the immediate children of `path`, directories first then files,
/// each group sorted ascending case-insensitively by name.
pub fn list_directory(path: &Path) -> Result<Vec<DirEntry>, FsError> {
    if path.is_file() {
        return Err(FsError::NotADirectory(path.to_string_lossy().into_owned()));
    }

    let mut entries: Vec<DirEntry> = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry
            .file_name()
            .to_str()
            .ok_or(FsError::InvalidUtf8)?
            .to_string();
        let entry_path = entry
            .path()
            .to_str()
            .ok_or(FsError::InvalidUtf8)?
            .to_string();
        entries.push(DirEntry {
            name,
            path: entry_path,
            is_dir: file_type.is_dir(),
        });
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

/// Reads `path` as UTF-8 text. Returns `FsError::BinaryFile` if the bytes are
/// not valid UTF-8, or `FsError::Io` on read failure.
pub fn read_file(path: &Path) -> Result<String, FsError> {
    let bytes = std::fs::read(path)?;
    String::from_utf8(bytes).map_err(|_| FsError::BinaryFile)
}

/// Writes `content` to `path` atomically: write to `<path>.tmp`, then rename
/// over `path`. A crash mid-write leaves the original intact and no partial
/// file at `path`; on success no `.tmp` artefact remains (Property 3).
pub fn write_file(path: &Path, content: &str) -> Result<(), FsError> {
    let mut tmp_path = path.as_os_str().to_owned();
    tmp_path.push(".tmp");
    let tmp_path = std::path::PathBuf::from(tmp_path);

    std::fs::write(&tmp_path, content)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Workspace-scoped mutations (Milestone 9 — Context Menu System)
//
// Every right-click file operation routes through these. They reject any target
// that resolves outside `workspace_root` (or that *is* the root) so a malicious
// or buggy caller can never mutate files beyond the open project (Requirement
// 5.3 / 8.4). Resolution canonicalizes through symlinks and `..` so traversal
// can't escape; for paths that don't exist yet (a rename/duplicate destination)
// the parent is canonicalized and the final component re-appended.
// ---------------------------------------------------------------------------

/// Canonicalize `path` for a containment check. Existing paths canonicalize
/// directly. For a not-yet-existing path (a create/rename/duplicate target,
/// possibly several levels deep), the nearest existing ancestor is canonicalized
/// and the remaining components are folded on — resolving `.`/`..` lexically so a
/// `..` in the not-yet-existing tail can't sneak the target out of the root.
fn resolve_for_check(path: &Path) -> Result<PathBuf, FsError> {
    if path.exists() {
        return path.canonicalize().map_err(FsError::from);
    }

    // Walk up to the deepest ancestor that actually exists on disk.
    let mut base = path;
    while !base.exists() {
        match base.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => base = parent,
            _ => return Err(FsError::OutsideWorkspace),
        }
    }

    // `base` is a real prefix of `path`; fold the remaining tail onto its
    // canonical form (canonicalizing `base` collapses any symlinks/`..` in it).
    let tail = path
        .strip_prefix(base)
        .map_err(|_| FsError::OutsideWorkspace)?;
    let mut resolved = base.canonicalize()?;
    for component in tail.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return Err(FsError::OutsideWorkspace);
                }
            }
            Component::Normal(c) => resolved.push(c),
            // A root/prefix component inside the tail is unexpected — reject.
            Component::RootDir | Component::Prefix(_) => return Err(FsError::OutsideWorkspace),
        }
    }
    Ok(resolved)
}

/// Ensure `path` resolves strictly *inside* `workspace_root`. The root itself is
/// rejected — these operations never act on the workspace folder as a whole.
fn ensure_within_workspace(path: &Path, workspace_root: &Path) -> Result<(), FsError> {
    let root = workspace_root.canonicalize()?;
    let target = resolve_for_check(path)?;
    if target != root && target.starts_with(&root) {
        Ok(())
    } else {
        Err(FsError::OutsideWorkspace)
    }
}

/// Public containment check for non-mutating callers (e.g. reveal-in-explorer):
/// `Ok(())` iff `path` is inside `workspace_root` (root excluded).
pub fn check_within_workspace(path: &Path, workspace_root: &Path) -> Result<(), FsError> {
    ensure_within_workspace(path, workspace_root)
}

/// Create an empty file at `path` (workspace-scoped). Errors if it already
/// exists so a New File never clobbers an existing one.
pub fn create_file(path: &Path, workspace_root: &Path) -> Result<(), FsError> {
    ensure_within_workspace(path, workspace_root)?;
    if path.exists() {
        return Err(FsError::AlreadyExists(path.to_string_lossy().into_owned()));
    }
    // create_new fails if the file already exists (guards against a TOCTOU race).
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    Ok(())
}

/// Create a directory at `path` (workspace-scoped, including missing parents).
/// Errors if it already exists.
pub fn create_dir(path: &Path, workspace_root: &Path) -> Result<(), FsError> {
    ensure_within_workspace(path, workspace_root)?;
    if path.exists() {
        return Err(FsError::AlreadyExists(path.to_string_lossy().into_owned()));
    }
    std::fs::create_dir_all(path)?;
    Ok(())
}

/// Rename/move `old` to `new` (both workspace-scoped). Errors if `old` is
/// missing or `new` already exists.
pub fn rename_path(old: &Path, new: &Path, workspace_root: &Path) -> Result<(), FsError> {
    ensure_within_workspace(old, workspace_root)?;
    ensure_within_workspace(new, workspace_root)?;
    if !old.exists() {
        return Err(FsError::NotFound(old.to_string_lossy().into_owned()));
    }
    if new.exists() {
        return Err(FsError::AlreadyExists(new.to_string_lossy().into_owned()));
    }
    std::fs::rename(old, new)?;
    Ok(())
}

/// Delete `path` (workspace-scoped). Recursively removes directories. Errors if
/// the path does not exist.
pub fn delete_path(path: &Path, workspace_root: &Path) -> Result<(), FsError> {
    ensure_within_workspace(path, workspace_root)?;
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| FsError::NotFound(path.to_string_lossy().into_owned()))?;
    if meta.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Duplicate `path` next to itself with a unique " copy" name (workspace-scoped).
/// Files are copied; directories are copied recursively. Returns the new path.
pub fn duplicate_path(path: &Path, workspace_root: &Path) -> Result<String, FsError> {
    ensure_within_workspace(path, workspace_root)?;
    if !path.exists() {
        return Err(FsError::NotFound(path.to_string_lossy().into_owned()));
    }
    let dest = unique_copy_path(path);
    ensure_within_workspace(&dest, workspace_root)?;

    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() {
        copy_dir_recursive(path, &dest)?;
    } else {
        std::fs::copy(path, &dest)?;
    }
    dest.to_str()
        .map(|s| s.to_string())
        .ok_or(FsError::InvalidUtf8)
}

/// Pick a non-colliding "<name> copy[ N]" sibling path for `path`.
fn unique_copy_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|s| s.to_str());
    let build = |suffix: &str| -> PathBuf {
        let name = match ext {
            Some(e) => format!("{stem}{suffix}.{e}"),
            None => format!("{stem}{suffix}"),
        };
        parent.join(name)
    };

    let mut candidate = build(" copy");
    let mut n = 2;
    while candidate.exists() {
        candidate = build(&format!(" copy {n}"));
        n += 1;
    }
    candidate
}

/// Recursively copy a directory tree from `src` to `dst`.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), FsError> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn list_directory_on_regular_file_returns_not_a_directory() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("plain.txt");
        fs::write(&file_path, b"hello").unwrap();

        match list_directory(&file_path) {
            Err(FsError::NotADirectory(_)) => {}
            other => panic!("expected NotADirectory, got {other:?}"),
        }
    }

    #[test]
    fn list_directory_sorts_dirs_first_then_case_insensitive() {
        let dir = tempdir().unwrap();
        // Mixed entries with deliberately unsorted, mixed-case names.
        fs::create_dir(dir.path().join("Zeta")).unwrap();
        fs::create_dir(dir.path().join("alpha")).unwrap();
        fs::write(dir.path().join("Beta.txt"), b"x").unwrap();
        fs::write(dir.path().join("apple.txt"), b"x").unwrap();

        let entries = list_directory(dir.path()).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        // Dirs first (case-insensitive), then files (case-insensitive).
        assert_eq!(names, vec!["alpha", "Zeta", "apple.txt", "Beta.txt"]);
    }

    // Property 1: list_directory sort order — dirs before files, each group
    // ascending case-insensitively by name.
    proptest! {
        #[test]
        fn prop_list_directory_sort_order(
            dir_names in proptest::collection::hash_set("[a-zA-Z]{1,8}", 0..8),
            file_names in proptest::collection::hash_set("[a-zA-Z]{1,8}", 0..8),
        ) {
            let dir = tempdir().unwrap();
            // Names that collide case-insensitively between the two groups would
            // create same-named dir+file on case-insensitive filesystems; keep
            // groups disjoint by suffixing files with an extension.
            for d in &dir_names {
                let _ = fs::create_dir(dir.path().join(d));
            }
            for f in &file_names {
                let _ = fs::write(dir.path().join(format!("{f}.dat")), b"x");
            }

            let entries = list_directory(dir.path()).unwrap();

            // All dirs precede all files.
            let first_file = entries.iter().position(|e| !e.is_dir);
            if let Some(idx) = first_file {
                prop_assert!(entries[idx..].iter().all(|e| !e.is_dir));
            }

            // Within each group, ascending case-insensitive by name.
            let dirs: Vec<String> = entries.iter().filter(|e| e.is_dir).map(|e| e.name.to_lowercase()).collect();
            let files: Vec<String> = entries.iter().filter(|e| !e.is_dir).map(|e| e.name.to_lowercase()).collect();
            let mut sorted_dirs = dirs.clone(); sorted_dirs.sort();
            let mut sorted_files = files.clone(); sorted_files.sort();
            prop_assert_eq!(dirs, sorted_dirs);
            prop_assert_eq!(files, sorted_files);
        }
    }

    // Property 5: list_directory on a regular file returns NotADirectory and
    // never panics.
    proptest! {
        #[test]
        fn prop_list_directory_on_file_is_not_a_directory(content in proptest::collection::vec(any::<u8>(), 0..64)) {
            let dir = tempdir().unwrap();
            let file_path = dir.path().join("f.bin");
            fs::write(&file_path, &content).unwrap();
            prop_assert!(matches!(list_directory(&file_path), Err(FsError::NotADirectory(_))));
        }
    }

    #[test]
    fn read_file_preserves_utf8_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("text.txt");
        let content = "hello\nworld — café 日本語\n";
        fs::write(&path, content).unwrap();
        assert_eq!(read_file(&path).unwrap(), content);
    }

    #[test]
    fn read_file_on_invalid_utf8_returns_binary_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("blob.bin");
        // 0xFF, 0xFE are never valid as standalone UTF-8.
        fs::write(&path, [0x00, 0xFF, 0xFE, 0x80, 0x01]).unwrap();
        assert!(matches!(read_file(&path), Err(FsError::BinaryFile)));
    }

    #[test]
    fn read_file_on_nonexistent_path_returns_io() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist.txt");
        assert!(matches!(read_file(&path), Err(FsError::Io(_))));
    }

    // Property 4: read_file on bytes that are not valid UTF-8 returns
    // BinaryFile and never panics. We prefix with a lone 0xFF, which can never
    // begin a valid UTF-8 sequence, guaranteeing the bytes are non-UTF-8.
    proptest! {
        #[test]
        fn prop_read_file_on_binary_returns_binary_file(tail in proptest::collection::vec(any::<u8>(), 0..64)) {
            let dir = tempdir().unwrap();
            let path = dir.path().join("b.bin");
            let mut bytes = vec![0xFFu8];
            bytes.extend_from_slice(&tail);
            fs::write(&path, &bytes).unwrap();
            prop_assert!(matches!(read_file(&path), Err(FsError::BinaryFile)));
        }
    }

    // Property 2 (partial): for any ASCII string written to disk, read_file
    // returns it exactly. The full write_file/read_file round-trip lands in
    // Wave 3 once write_file exists; here we verify the read half.
    proptest! {
        #[test]
        fn prop_read_file_roundtrip_ascii(s in "[\\x20-\\x7E]*") {
            let dir = tempdir().unwrap();
            let path = dir.path().join("ascii.txt");
            fs::write(&path, &s).unwrap();
            prop_assert_eq!(read_file(&path).unwrap(), s);
        }
    }

    #[test]
    fn write_then_read_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("out.txt");
        let content = "line one\nline two — αβγ\n";
        write_file(&path, content).unwrap();
        assert_eq!(read_file(&path).unwrap(), content);
    }

    #[test]
    fn write_file_leaves_no_tmp_artefact() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.txt");
        write_file(&path, "hello").unwrap();

        let mut tmp = path.clone().into_os_string();
        tmp.push(".tmp");
        assert!(
            !std::path::Path::new(&tmp).exists(),
            ".tmp artefact must not remain after success"
        );
        assert!(path.exists());
    }

    // Property 2: write_file then read_file preserves any UTF-8 string exactly.
    proptest! {
        #[test]
        fn prop_write_read_roundtrip(s in ".*") {
            let dir = tempdir().unwrap();
            let path = dir.path().join("rt.txt");
            write_file(&path, &s).unwrap();
            prop_assert_eq!(read_file(&path).unwrap(), s);
        }
    }

    // Property 3: after a successful write_file, no `<path>.tmp` remains.
    proptest! {
        #[test]
        fn prop_write_file_no_tmp_artefact(s in ".*") {
            let dir = tempdir().unwrap();
            let path = dir.path().join("p.txt");
            write_file(&path, &s).unwrap();
            let mut tmp = path.clone().into_os_string();
            tmp.push(".tmp");
            prop_assert!(!std::path::Path::new(&tmp).exists());
        }
    }

    // --- Workspace-scoped mutations (Milestone 9) --------------------------

    #[test]
    fn create_file_makes_empty_file_inside_workspace() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("new.txt");
        create_file(&path, dir.path()).unwrap();
        assert!(path.is_file());
        assert_eq!(read_file(&path).unwrap(), "");
    }

    #[test]
    fn create_file_rejects_existing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("dup.txt");
        fs::write(&path, b"x").unwrap();
        assert!(matches!(
            create_file(&path, dir.path()),
            Err(FsError::AlreadyExists(_))
        ));
    }

    #[test]
    fn create_dir_makes_directory_inside_workspace() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested/folder");
        create_dir(&path, dir.path()).unwrap();
        assert!(path.is_dir());
    }

    #[test]
    fn rename_moves_within_workspace() {
        let dir = tempdir().unwrap();
        let old = dir.path().join("a.txt");
        let new = dir.path().join("b.txt");
        fs::write(&old, b"hi").unwrap();
        rename_path(&old, &new, dir.path()).unwrap();
        assert!(!old.exists());
        assert_eq!(read_file(&new).unwrap(), "hi");
    }

    #[test]
    fn rename_rejects_existing_destination() {
        let dir = tempdir().unwrap();
        let old = dir.path().join("a.txt");
        let new = dir.path().join("b.txt");
        fs::write(&old, b"a").unwrap();
        fs::write(&new, b"b").unwrap();
        assert!(matches!(
            rename_path(&old, &new, dir.path()),
            Err(FsError::AlreadyExists(_))
        ));
    }

    #[test]
    fn delete_removes_file_and_dir() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("f.txt");
        fs::write(&file, b"x").unwrap();
        delete_path(&file, dir.path()).unwrap();
        assert!(!file.exists());

        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("child.txt"), b"x").unwrap();
        delete_path(&sub, dir.path()).unwrap();
        assert!(!sub.exists());
    }

    #[test]
    fn duplicate_file_creates_copy_sibling() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("note.md");
        fs::write(&file, b"content").unwrap();
        let copy = duplicate_path(&file, dir.path()).unwrap();
        assert_eq!(
            std::path::Path::new(&copy).file_name().unwrap(),
            "note copy.md"
        );
        assert_eq!(read_file(std::path::Path::new(&copy)).unwrap(), "content");
        // A second duplicate of the original gets a numbered suffix.
        let copy2 = duplicate_path(&file, dir.path()).unwrap();
        assert_eq!(
            std::path::Path::new(&copy2).file_name().unwrap(),
            "note copy 2.md"
        );
    }

    #[test]
    fn duplicate_directory_copies_recursively() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("pkg");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"a").unwrap();
        let copy = duplicate_path(&src, dir.path()).unwrap();
        let copy_path = std::path::Path::new(&copy);
        assert_eq!(copy_path.file_name().unwrap(), "pkg copy");
        assert_eq!(read_file(&copy_path.join("a.txt")).unwrap(), "a");
    }

    #[test]
    fn operations_reject_paths_outside_workspace() {
        let ws = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let outside_file = outside.path().join("secret.txt");
        fs::write(&outside_file, b"secret").unwrap();

        // Delete/rename/duplicate of an absolute path outside the root are rejected.
        assert!(matches!(
            delete_path(&outside_file, ws.path()),
            Err(FsError::OutsideWorkspace)
        ));
        assert!(matches!(
            rename_path(&outside_file, &outside.path().join("x.txt"), ws.path()),
            Err(FsError::OutsideWorkspace)
        ));
        assert!(matches!(
            duplicate_path(&outside_file, ws.path()),
            Err(FsError::OutsideWorkspace)
        ));
        // The still-exists invariant: a rejected delete never touched the file.
        assert!(outside_file.exists());
    }

    #[test]
    fn traversal_escape_is_rejected() {
        let ws = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let outside_file = outside.path().join("target.txt");
        fs::write(&outside_file, b"x").unwrap();
        // A `..`-laden path that climbs out of the workspace is rejected even
        // though it is expressed relative to the root.
        let escape = ws
            .path()
            .join("..")
            .join(outside.path().file_name().unwrap())
            .join("target.txt");
        assert!(matches!(
            delete_path(&escape, ws.path()),
            Err(FsError::OutsideWorkspace)
        ));
        assert!(outside_file.exists());
    }

    #[test]
    fn deleting_workspace_root_itself_is_rejected() {
        let ws = tempdir().unwrap();
        assert!(matches!(
            delete_path(ws.path(), ws.path()),
            Err(FsError::OutsideWorkspace)
        ));
        assert!(ws.path().exists());
    }
}
