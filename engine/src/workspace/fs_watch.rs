//! Batched, polling-based file-system watcher (M19 Wave 1, GWEN-376).
//!
//! Why polling (not `notify`/inotify/ReadDirectoryChangesW)?
//! - The project's hard constraint is ZERO new crates, so `notify` is out.
//! - A from-scratch native watcher means per-platform `unsafe` FFI
//!   (ReadDirectoryChangesW + inotify + FSEvents) — far too much surface for a
//!   single wave. A polling diff is simple, cross-platform, and zero-dep.
//!
//! Design rules (mirrors the M19 plan's FS-event pipeline):
//! - **Watch only registered directories.** The UI registers each folder it has
//!   expanded/visible; we never recurse the whole tree. On an i3/8GB target this
//!   keeps each poll to a handful of `read_dir` calls, not a workspace walk.
//! - **The poll interval IS the coalescing window.** A burst of FS activity
//!   (e.g. `npm install` writing thousands of files) is collapsed into at most
//!   one [`FsPatch`] per affected directory per cycle — never one event per file.
//! - **Diff against the previous snapshot** (name + is_dir + mtime + len) to
//!   classify each change as added / removed / modified.
//! - **`.git` internals are ignored** so commits/index churn don't spam the UI.
//! - Tauri-free: the watcher takes a callback `Fn(Vec<FsPatch>)`, exactly like
//!   `PtySession` takes an output callback. The Tauri side turns the callback
//!   into an `fs:patch` event.

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Default poll cadence. 600ms is a deliberate compromise for the slow-disk /
/// low-core target: long enough to coalesce an `npm install` burst into a
/// handful of patches, short enough that a single manual create/delete feels
/// immediate. Tests override this for determinism.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(600);

/// One coalesced batch of changes for a single directory. Emitted as the
/// `fs:patch` payload (the Tauri side sends `Vec<FsPatch>`). Each list holds
/// absolute child paths; `dir` is the absolute parent directory affected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FsPatch {
    pub dir: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
    pub modified_dirs: Vec<String>,
}

impl FsPatch {
    fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.modified.is_empty()
            && self.modified_dirs.is_empty()
    }
}

/// A child entry's identity fingerprint within a directory snapshot. Two entries
/// with the same key but a different fingerprint count as "modified".
#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryStat {
    is_dir: bool,
    /// File size; folders report 0 (their mtime is the meaningful signal).
    len: u64,
    /// Modification time as nanos since the epoch, or 0 if unavailable. A bumped
    /// mtime is what distinguishes an in-place edit from a no-op poll.
    mtime_nanos: u128,
}

/// Snapshot of one directory's immediate children, keyed by absolute path.
type DirSnapshot = HashMap<String, EntryStat>;

/// Default generated/dependency paths that must never be watched or reported.
/// TODO(M21): allow a project-level override under `.gwenland/settings.json`.
const DEFAULT_EXCLUDES: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    "out",
    "coverage",
    ".cache",
    "__pycache__",
    ".DS_Store",
];

fn is_excluded_name(name: &str) -> bool {
    DEFAULT_EXCLUDES
        .iter()
        .any(|excluded| name.eq_ignore_ascii_case(excluded))
}

/// Returns `true` if `path` lives inside a default excluded component. Used both
/// to refuse watch registration and to drop entries from parent snapshots.
fn should_exclude(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => name.to_str().map(is_excluded_name).unwrap_or(false),
        _ => false,
    })
}

fn entry_stat(meta: &std::fs::Metadata) -> EntryStat {
    let mtime_nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    EntryStat {
        is_dir: meta.is_dir(),
        len: if meta.is_dir() { 0 } else { meta.len() },
        mtime_nanos,
    }
}

/// Read the immediate children of `dir` into a snapshot. Unreadable entries are
/// skipped (a transient permission error mid-`npm install` must not kill the
/// poll loop). Returns `None` only if the directory itself can't be read.
fn snapshot_dir(dir: &Path) -> Option<DirSnapshot> {
    let read = std::fs::read_dir(dir).ok()?;
    let mut snap = DirSnapshot::new();
    for entry in read.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if should_exclude(&entry.path()) {
            continue;
        }
        let Some(path) = entry.path().to_str().map(str::to_owned) else {
            continue;
        };
        snap.insert(path, entry_stat(&meta));
    }
    Some(snap)
}

/// Diff `old` → `new` for one directory into an `FsPatch`. Pure; unit-tested
/// directly. An entry present in both with a changed fingerprint is "modified".
fn diff_snapshots(dir: &str, old: &DirSnapshot, new: &DirSnapshot) -> FsPatch {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let mut modified_dirs = Vec::new();

    for (path, stat) in new {
        match old.get(path) {
            None => added.push(path.clone()),
            Some(prev) if prev != stat && stat.is_dir => modified_dirs.push(path.clone()),
            Some(prev) if prev != stat => modified.push(path.clone()),
            Some(_) => {}
        }
    }
    for path in old.keys() {
        if !new.contains_key(path) {
            removed.push(path.clone());
        }
    }

    added.sort();
    removed.sort();
    modified.sort();
    modified_dirs.sort();
    FsPatch {
        dir: dir.to_string(),
        added,
        removed,
        modified,
        modified_dirs,
    }
}

/// Shared watch state: the set of registered directories and their last-seen
/// snapshots. Guarded by a single mutex (poll cadence is coarse, so contention
/// is negligible).
#[derive(Default)]
struct WatchState {
    /// Registered directory → its last snapshot. Absence of a key means "not
    /// watched". A freshly registered dir gets an empty snapshot so its current
    /// contents are NOT reported as a spurious "added" burst on first poll
    /// (we seed it synchronously at registration time instead).
    dirs: HashMap<String, DirSnapshot>,
}

/// A running polling watcher. Owns a background thread that wakes every
/// `interval`, diffs every registered directory, and invokes the callback with
/// one coalesced `Vec<FsPatch>` (empty batches are suppressed). Dropping the
/// handle stops the thread.
pub struct FsWatcher {
    state: Arc<Mutex<WatchState>>,
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FsWatcher {
    /// Start a watcher with the default poll interval.
    pub fn start<F>(callback: F) -> Self
    where
        F: Fn(Vec<FsPatch>) + Send + 'static,
    {
        Self::start_with_interval(DEFAULT_POLL_INTERVAL, callback)
    }

    /// Start a watcher with an explicit poll interval (tests use a short one).
    pub fn start_with_interval<F>(interval: Duration, callback: F) -> Self
    where
        F: Fn(Vec<FsPatch>) + Send + 'static,
    {
        let state: Arc<Mutex<WatchState>> = Arc::new(Mutex::new(WatchState::default()));
        let running = Arc::new(AtomicBool::new(true));

        let thread_state = state.clone();
        let thread_running = running.clone();
        let handle = std::thread::spawn(move || {
            while thread_running.load(Ordering::Relaxed) {
                std::thread::park_timeout(interval);
                if !thread_running.load(Ordering::Relaxed) {
                    break;
                }
                let patches = poll_once(&thread_state);
                if !patches.is_empty() {
                    callback(patches);
                }
            }
        });

        Self {
            state,
            running,
            handle: Some(handle),
        }
    }

    /// Begin watching `dir`. Its current contents are snapshotted immediately so
    /// the first poll reports only *subsequent* changes, never the existing
    /// children as a fake "added" flood. Re-registering a watched dir re-seeds
    /// its snapshot (cheap, idempotent). `.git` and paths inside it are refused.
    pub fn watch(&self, dir: &str) {
        let path = PathBuf::from(dir);
        if should_exclude(&path) {
            return;
        }
        let seed = snapshot_dir(&path).unwrap_or_default();
        if let Ok(mut state) = self.state.lock() {
            state.dirs.insert(dir.to_string(), seed);
        }
    }

    /// Stop watching `dir` (e.g. the user collapsed the folder). Idempotent.
    pub fn unwatch(&self, dir: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.dirs.remove(dir);
        }
    }

    /// Stop watching everything (e.g. workspace closed/switched).
    pub fn clear(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.dirs.clear();
        }
    }

    /// Number of directories currently registered (test/diagnostic helper).
    pub fn watched_count(&self) -> usize {
        self.state.lock().map(|s| s.dirs.len()).unwrap_or(0)
    }
}

impl Drop for FsWatcher {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }
}

/// Run one poll cycle: re-snapshot every registered dir, diff against the stored
/// snapshot, update the store, and return all non-empty patches. Factored out so
/// tests can drive a single deterministic cycle without the timing thread.
fn poll_once(state: &Arc<Mutex<WatchState>>) -> Vec<FsPatch> {
    // Snapshot the registered key set under the lock, then release it for the
    // (potentially slow) read_dir calls so registration isn't blocked on I/O.
    let dirs: Vec<String> = match state.lock() {
        Ok(s) => s.dirs.keys().cloned().collect(),
        Err(_) => return Vec::new(),
    };

    let mut patches = Vec::new();
    for dir in dirs {
        let path = PathBuf::from(&dir);
        let Some(fresh) = snapshot_dir(&path) else {
            // Directory vanished (deleted/renamed). Drop it from the watch set;
            // the parent dir's own patch (if watched) reports the removal.
            if let Ok(mut s) = state.lock() {
                s.dirs.remove(&dir);
            }
            continue;
        };

        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(_) => continue,
        };
        // The dir may have been unwatched while we read it; if so, skip.
        let Some(prev) = guard.dirs.get(&dir) else {
            continue;
        };
        let patch = diff_snapshots(&dir, prev, &fresh);
        guard.dirs.insert(dir.clone(), fresh);
        drop(guard);

        if !patch.is_empty() {
            patches.push(patch);
        }
    }
    patches
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn snap_state(dir: &str) -> Arc<Mutex<WatchState>> {
        let state = Arc::new(Mutex::new(WatchState::default()));
        let seed = snapshot_dir(&PathBuf::from(dir)).unwrap_or_default();
        state.lock().unwrap().dirs.insert(dir.to_string(), seed);
        state
    }

    // 1.1 — a single file create produces exactly one patch with one `added`.
    #[test]
    fn single_create_one_patch() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root);

        fs::write(dir.path().join("a.txt"), b"hi").unwrap();
        let patches = poll_once(&state);

        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].added.len(), 1);
        assert!(patches[0].added[0].ends_with("a.txt"));
        assert!(patches[0].removed.is_empty());
        assert!(patches[0].modified.is_empty());
    }

    // 1.2 — a burst of many creates coalesces into ONE patch (the npm-install
    // case): never one event per file.
    #[test]
    fn burst_coalesces_into_one_patch() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root);

        for i in 0..1000 {
            fs::write(dir.path().join(format!("f{i}.txt")), b"x").unwrap();
        }
        let patches = poll_once(&state);

        assert_eq!(patches.len(), 1, "1000 creates must be ONE patch");
        assert_eq!(patches[0].added.len(), 1000);
    }

    // 1.3 — delete is reported as `removed`.
    #[test]
    fn delete_reported_as_removed() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("gone.txt");
        fs::write(&f, b"x").unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root);

        fs::remove_file(&f).unwrap();
        let patches = poll_once(&state);

        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].removed.len(), 1);
        assert!(patches[0].removed[0].ends_with("gone.txt"));
    }

    // 1.4 — an in-place content edit (changed size/mtime) is `modified`.
    #[test]
    fn edit_reported_as_modified() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("edit.txt");
        fs::write(&f, b"short").unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root);

        fs::write(&f, b"a much longer body than before").unwrap();
        let patches = poll_once(&state);

        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].modified.len(), 1);
        assert!(patches[0].modified[0].ends_with("edit.txt"));
    }

    // 1.5 — no change ⇒ no patch (the steady-state poll must be silent).
    #[test]
    fn no_change_no_patch() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("stable.txt"), b"x").unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root);

        let patches = poll_once(&state);
        assert!(patches.is_empty());
    }

    // 1.6 — `.git` children are never reported even when the parent is watched.
    #[test]
    fn git_internal_files_ignored() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root);

        // Writing inside .git must not produce a patch (the .git dir itself is
        // filtered out of snapshots, so it never even appears).
        fs::write(dir.path().join(".git/index"), b"x").unwrap();
        let patches = poll_once(&state);
        assert!(patches.is_empty());
    }

    // 1.6b — dependency/generated dirs are never reported from a watched parent.
    #[test]
    fn default_excluded_dirs_are_ignored() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root);

        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules").join("pkg.js"), b"x").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target").join("debug.log"), b"x").unwrap();

        let patches = poll_once(&state);
        assert!(patches.is_empty());
    }

    // 1.6c — excluded files such as .DS_Store are also dropped.
    #[test]
    fn default_excluded_files_are_ignored() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root);

        fs::write(dir.path().join(".DS_Store"), b"x").unwrap();
        let patches = poll_once(&state);

        assert!(patches.is_empty());
    }

    // 1.7 — registering a dir does NOT report its existing contents as added.
    #[test]
    fn registration_does_not_flood_existing_contents() {
        let dir = tempdir().unwrap();
        for i in 0..5 {
            fs::write(dir.path().join(format!("pre{i}.txt")), b"x").unwrap();
        }
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root); // seeds with the 5 existing files

        let patches = poll_once(&state);
        assert!(
            patches.is_empty(),
            "existing files must not appear as added"
        );
    }

    // 1.8 — diff_snapshots classifies add/remove/modify together correctly.
    #[test]
    fn diff_mixed_changes() {
        let mut old = DirSnapshot::new();
        old.insert(
            "keep".into(),
            EntryStat {
                is_dir: false,
                len: 1,
                mtime_nanos: 1,
            },
        );
        old.insert(
            "edit".into(),
            EntryStat {
                is_dir: false,
                len: 1,
                mtime_nanos: 1,
            },
        );
        old.insert(
            "del".into(),
            EntryStat {
                is_dir: false,
                len: 1,
                mtime_nanos: 1,
            },
        );

        let mut new = DirSnapshot::new();
        new.insert(
            "keep".into(),
            EntryStat {
                is_dir: false,
                len: 1,
                mtime_nanos: 1,
            },
        );
        new.insert(
            "edit".into(),
            EntryStat {
                is_dir: false,
                len: 2,
                mtime_nanos: 9,
            },
        );
        new.insert(
            "new".into(),
            EntryStat {
                is_dir: false,
                len: 1,
                mtime_nanos: 1,
            },
        );

        let patch = diff_snapshots("d", &old, &new);
        assert_eq!(patch.added, vec!["new".to_string()]);
        assert_eq!(patch.removed, vec!["del".to_string()]);
        assert_eq!(patch.modified, vec!["edit".to_string()]);
        assert!(patch.modified_dirs.is_empty());
    }

    // 1.8b — a changed directory fingerprint is separated so the UI can mark
    // collapsed folders stale without reconciling content-only file saves.
    #[test]
    fn diff_directory_modification_separate_from_file_modification() {
        let mut old = DirSnapshot::new();
        old.insert(
            "dir".into(),
            EntryStat {
                is_dir: true,
                len: 0,
                mtime_nanos: 1,
            },
        );

        let mut new = DirSnapshot::new();
        new.insert(
            "dir".into(),
            EntryStat {
                is_dir: true,
                len: 0,
                mtime_nanos: 2,
            },
        );

        let patch = diff_snapshots("root", &old, &new);
        assert!(patch.modified.is_empty());
        assert_eq!(patch.modified_dirs, vec!["dir".to_string()]);
    }

    // 1.9 — two separate watched dirs each get their own patch.
    #[test]
    fn separate_dirs_separate_patches() {
        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        let a_root = a.path().to_str().unwrap().to_string();
        let b_root = b.path().to_str().unwrap().to_string();
        let state = Arc::new(Mutex::new(WatchState::default()));
        {
            let mut g = state.lock().unwrap();
            g.dirs
                .insert(a_root.clone(), snapshot_dir(a.path()).unwrap());
            g.dirs
                .insert(b_root.clone(), snapshot_dir(b.path()).unwrap());
        }

        fs::write(a.path().join("x.txt"), b"x").unwrap();
        fs::write(b.path().join("y.txt"), b"y").unwrap();
        let patches = poll_once(&state);

        assert_eq!(patches.len(), 2);
    }

    // 1.10 — unwatch removes a dir so its changes stop being reported.
    #[test]
    fn unwatch_stops_reporting() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root);
        state.lock().unwrap().dirs.remove(&root);

        fs::write(dir.path().join("z.txt"), b"x").unwrap();
        let patches = poll_once(&state);
        assert!(patches.is_empty());
    }

    // 1.11 — a deleted watched directory is dropped from the watch set.
    #[test]
    fn vanished_dir_is_dropped() {
        let parent = tempdir().unwrap();
        let sub = parent.path().join("sub");
        fs::create_dir(&sub).unwrap();
        let sub_root = sub.to_str().unwrap().to_string();
        let state = snap_state(&sub_root);
        assert_eq!(state.lock().unwrap().dirs.len(), 1);

        fs::remove_dir_all(&sub).unwrap();
        let patches = poll_once(&state);
        assert!(patches.is_empty());
        assert_eq!(
            state.lock().unwrap().dirs.len(),
            0,
            "vanished dir must be unwatched"
        );
    }

    // 1.12 — watcher refuses to register a path inside .git.
    #[test]
    fn watch_refuses_git_paths() {
        let watcher = FsWatcher::start_with_interval(Duration::from_secs(3600), |_| {});
        watcher.watch("/some/repo/.git/refs");
        assert_eq!(watcher.watched_count(), 0);
    }

    // 1.12b — watcher refuses generated/dependency folders too.
    #[test]
    fn watch_refuses_default_excluded_paths() {
        let watcher = FsWatcher::start_with_interval(Duration::from_secs(3600), |_| {});
        watcher.watch("/some/repo/node_modules/react");
        watcher.watch("/some/repo/target/debug");
        assert_eq!(watcher.watched_count(), 0);
    }

    #[test]
    fn excludes_are_case_insensitive() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let state = snap_state(&root);

        fs::create_dir(dir.path().join("Node_Modules")).unwrap();
        fs::write(dir.path().join("Node_Modules").join("pkg.js"), b"x").unwrap();

        let patches = poll_once(&state);
        assert!(patches.is_empty());
    }

    #[test]
    fn drop_wakes_long_interval_watcher() {
        let watcher = FsWatcher::start_with_interval(Duration::from_secs(3600), |_| {});
        std::thread::sleep(Duration::from_millis(10));
        drop(watcher);
    }

    // 1.13 — end-to-end: the live thread delivers a coalesced patch via callback.
    #[test]
    fn live_thread_delivers_patch() {
        use std::sync::mpsc;
        let dir = tempdir().unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let (tx, rx) = mpsc::channel();
        let watcher = FsWatcher::start_with_interval(Duration::from_millis(40), move |patches| {
            let _ = tx.send(patches);
        });
        watcher.watch(&root);

        fs::write(dir.path().join("late.txt"), b"x").unwrap();
        let patches = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("expected a patch from the live watcher");
        assert!(
            patches
                .iter()
                .any(|p| p.added.iter().any(|a| a.ends_with("late.txt")))
        );
    }
}
