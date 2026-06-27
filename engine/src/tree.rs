//! Rust-owned flat file-tree model (M19 Wave 2, GWEN-374 + GWEN-375).
//!
//! The UI no longer holds the tree shape — Rust does. The tree is kept as a
//! single ordered `Vec<FlatRow>` (depth-first, the exact render order) plus the
//! set of expanded directories. Every mutation returns `Vec<TreePatch>`
//! (Insert / Remove / Update) describing the delta, so the frontend never
//! receives the whole tree — only diffs it splices into its mirror array.
//!
//! Design rules:
//! - **Lazy** (GWEN-375): a folder's children are only listed when it expands.
//!   Root rows are listed on open; nested rows arrive as Insert patches.
//! - **Stable ids**: a row's id is its absolute path. The frontend keys on it
//!   for virtual-scroll reconciliation; collapsing+re-expanding yields the same
//!   ids.
//! - **No git / no icons here.** Git status comes from the git store and icons
//!   are derived from the filename, both already on the JS side. Duplicating
//!   them into the row model would create a second source of truth, so the row
//!   carries only structural data.
//! - Pure model: zero Tauri. The Tauri side wraps a `WorkspaceTree` in managed
//!   state and turns returned patches into `tree:patch` events.

use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

use crate::fs::{DirEntry, list_directory};

/// One row in the flattened, depth-first tree — the unit the UI renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlatRow {
    /// Stable unique id (the absolute path).
    pub id: String,
    pub name: String,
    pub path: String,
    /// Indentation depth; root entries are depth 0.
    pub depth: usize,
    pub is_dir: bool,
    /// True when this directory is currently expanded (its children follow).
    pub is_expanded: bool,
    /// Whether this directory has any children. For an unexpanded folder this is
    /// a cheap "is the dir non-empty" probe so the UI can show/hide the twistie
    /// without listing. Always false for files.
    pub has_children: bool,
}

/// A delta the UI applies to its mirror of the flat row array. Indices are into
/// the post-previous-patch array (patches in a `Vec` apply in order).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TreePatch {
    /// Insert `rows` starting at `index` (shifting later rows down).
    Insert { index: usize, rows: Vec<FlatRow> },
    /// Remove `count` rows starting at `index`.
    Remove { index: usize, count: usize },
    /// Replace the single row at `index` (e.g. twistie/has_children change).
    Update { index: usize, row: FlatRow },
}

/// The Rust-owned tree. Holds the ordered flat rows and the expanded-dir set.
/// All mutating methods return the patches that transform the *previous* row
/// array into the new one.
#[derive(Default)]
pub struct WorkspaceTree {
    root: Option<String>,
    rows: Vec<FlatRow>,
    expanded: HashSet<String>,
}

/// Normalize a path for comparison/membership (separator + trailing slash). The
/// stored ids keep their original form; this is only for matching.
fn norm(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// Probe whether `dir` has at least one child, without a full listing. Errors
/// (permission, gone) read as "no children" — the twistie just won't show.
fn dir_has_children(dir: &str) -> bool {
    std::fs::read_dir(dir)
        .map(|mut it| it.next().is_some())
        .unwrap_or(false)
}

fn row_from_entry(entry: &DirEntry, depth: usize, expanded: &HashSet<String>) -> FlatRow {
    let is_expanded = entry.is_dir && expanded.contains(&entry.path);
    let has_children = if entry.is_dir {
        dir_has_children(&entry.path)
    } else {
        false
    };
    FlatRow {
        id: entry.path.clone(),
        name: entry.name.clone(),
        path: entry.path.clone(),
        depth,
        is_dir: entry.is_dir,
        is_expanded,
        has_children,
    }
}

impl WorkspaceTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Current ordered rows (for the initial render after `set_root`).
    pub fn rows(&self) -> &[FlatRow] {
        &self.rows
    }

    pub fn root(&self) -> Option<&str> {
        self.root.as_deref()
    }

    /// Find a row's index by path (separator-insensitive).
    fn index_of(&self, path: &str) -> Option<usize> {
        let target = norm(path);
        self.rows.iter().position(|r| norm(&r.path) == target)
    }

    /// The index just past the subtree rooted at `start` (a row at depth d owns
    /// the contiguous run of following rows with depth > d). Used to size a
    /// collapse-remove and to find a folder's insertion point.
    fn subtree_end(&self, start: usize) -> usize {
        let base = self.rows[start].depth;
        let mut end = start + 1;
        while end < self.rows.len() && self.rows[end].depth > base {
            end += 1;
        }
        end
    }

    /// Open `root_path` and list its immediate children, replacing any prior
    /// tree. Returns the full row set (the only place a non-diff snapshot is
    /// handed out — it's the initial render). Expanded state is reset.
    pub fn set_root(&mut self, root_path: &str) -> Vec<FlatRow> {
        self.root = Some(root_path.to_string());
        self.expanded.clear();
        self.rows.clear();
        if let Ok(entries) = list_directory(Path::new(root_path)) {
            for entry in &entries {
                self.rows.push(row_from_entry(entry, 0, &self.expanded));
            }
        }
        self.rows.clone()
    }

    /// Expand the folder at `path`: list its children and splice them in right
    /// after the folder row. No-op (empty patch) if `path` is unknown, not a
    /// directory, or already expanded. Returns Insert + an Update flipping the
    /// folder's `is_expanded`.
    pub fn expand(&mut self, path: &str) -> Vec<TreePatch> {
        let Some(idx) = self.index_of(path) else {
            return Vec::new();
        };
        let row = &self.rows[idx];
        if !row.is_dir || row.is_expanded {
            return Vec::new();
        }
        let depth = row.depth;
        let dir_path = row.path.clone();

        let entries = match list_directory(Path::new(&dir_path)) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        self.expanded.insert(dir_path.clone());
        // Mark the folder expanded (Update) and recompute has_children.
        self.rows[idx].is_expanded = true;
        self.rows[idx].has_children = !entries.is_empty();
        let updated = self.rows[idx].clone();

        let child_rows: Vec<FlatRow> = entries
            .iter()
            .map(|e| row_from_entry(e, depth + 1, &self.expanded))
            .collect();

        let insert_at = idx + 1;
        for (offset, r) in child_rows.iter().enumerate() {
            self.rows.insert(insert_at + offset, r.clone());
        }

        vec![
            TreePatch::Update {
                index: idx,
                row: updated,
            },
            TreePatch::Insert {
                index: insert_at,
                rows: child_rows,
            },
        ]
    }

    /// Collapse the folder at `path`: remove its entire subtree run. Also drops
    /// every descendant from the expanded set so a later re-expand re-lists.
    /// No-op if unknown/not-a-dir/not-expanded. Returns Remove + Update.
    pub fn collapse(&mut self, path: &str) -> Vec<TreePatch> {
        let Some(idx) = self.index_of(path) else {
            return Vec::new();
        };
        let row = &self.rows[idx];
        if !row.is_dir || !row.is_expanded {
            return Vec::new();
        }

        let end = self.subtree_end(idx);
        let remove_at = idx + 1;
        let count = end - remove_at;

        // Forget expansion for the folder and all removed descendants.
        for r in &self.rows[remove_at..end] {
            self.expanded.remove(&r.path);
        }
        self.expanded.remove(&self.rows[idx].path);
        self.rows[idx].is_expanded = false;
        let updated = self.rows[idx].clone();

        self.rows.drain(remove_at..end);

        let mut patches = vec![TreePatch::Update {
            index: idx,
            row: updated,
        }];
        if count > 0 {
            patches.push(TreePatch::Remove {
                index: remove_at,
                count,
            });
        }
        patches
    }

    /// Re-list a single (visible) directory in place, producing the minimal
    /// patches to reconcile its *direct* children with disk. Used to apply a
    /// watcher `fs:patch` (create/delete/rename inside a watched, expanded dir).
    ///
    /// Only the directory's immediate children are reconciled; already-expanded
    /// grandchildren keep their subtrees (an Insert/Remove only touches the
    /// direct level). Unknown dir, not-a-dir, or not-expanded → empty.
    pub fn refresh_dir(&mut self, path: &str) -> Vec<TreePatch> {
        // Special-case the root: its children live at depth 0 with no folder row.
        if self.root.as_deref().map(norm) == Some(norm(path)) {
            return self.refresh_root();
        }

        let Some(idx) = self.index_of(path) else {
            return Vec::new();
        };
        if !self.rows[idx].is_dir || !self.rows[idx].is_expanded {
            // Not expanded: only its has_children twistie might change.
            return self.refresh_twistie(idx);
        }
        let depth = self.rows[idx].depth;
        let child_depth = depth + 1;
        let dir_path = self.rows[idx].path.clone();

        let entries = match list_directory(Path::new(&dir_path)) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        let new_rows: Vec<FlatRow> = entries
            .iter()
            .map(|e| row_from_entry(e, child_depth, &self.expanded))
            .collect();

        let start = idx + 1;
        let end = self.subtree_end(idx);
        self.reconcile_range(start, end, child_depth, new_rows)
    }

    /// Reconcile the root's depth-0 rows against disk (root has no folder row).
    fn refresh_root(&mut self) -> Vec<TreePatch> {
        let Some(root) = self.root.clone() else {
            return Vec::new();
        };
        let entries = match list_directory(Path::new(&root)) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        let new_rows: Vec<FlatRow> = entries
            .iter()
            .map(|e| row_from_entry(e, 0, &self.expanded))
            .collect();
        let end = self.rows.len();
        self.reconcile_range(0, end, 0, new_rows)
    }

    /// Update only the twistie/has_children flag of an unexpanded folder row.
    fn refresh_twistie(&mut self, idx: usize) -> Vec<TreePatch> {
        let has = dir_has_children(&self.rows[idx].path);
        if self.rows[idx].has_children == has {
            return Vec::new();
        }
        self.rows[idx].has_children = has;
        vec![TreePatch::Update {
            index: idx,
            row: self.rows[idx].clone(),
        }]
    }

    /// Reconcile the direct-child run `[start, old_end)` (all at `child_depth`,
    /// plus any expanded subtrees nested under them) against the freshly listed
    /// `new_rows` (direct children only). Produces Remove patches for vanished
    /// children (and their subtrees) and Insert patches for new children, in a
    /// single ordered merge. Surviving children keep their expanded subtrees
    /// untouched.
    fn reconcile_range(
        &mut self,
        start: usize,
        old_end: usize,
        child_depth: usize,
        new_rows: Vec<FlatRow>,
    ) -> Vec<TreePatch> {
        // Snapshot the existing DIRECT children (depth == child_depth) and the
        // span each occupies (child row + its expanded subtree).
        let mut old_children: Vec<(String, usize, usize)> = Vec::new(); // (norm path, start, end)
        let mut i = start;
        while i < old_end {
            if self.rows[i].depth == child_depth {
                let sub_end = self.subtree_end(i);
                old_children.push((norm(&self.rows[i].path), i, sub_end));
                i = sub_end;
            } else {
                i += 1; // defensive; shouldn't happen given subtree contiguity
            }
        }

        let new_keys: HashSet<String> = new_rows.iter().map(|r| norm(&r.path)).collect();
        let old_keys: HashSet<String> = old_children.iter().map(|(k, _, _)| k.clone()).collect();

        let mut patches: Vec<TreePatch> = Vec::new();

        // 1. Remove vanished children (highest index first so earlier indices
        //    stay valid as we mutate `self.rows`).
        for (key, c_start, c_end) in old_children.iter().rev() {
            if !new_keys.contains(key) {
                let count = c_end - c_start;
                self.rows.drain(*c_start..*c_end);
                patches.push(TreePatch::Remove {
                    index: *c_start,
                    count,
                });
            }
        }

        // 2. Insert new children at the correct sorted position. Disk order
        //    (dirs first, name asc) is authoritative; we walk `new_rows` and
        //    place any not already present just before the next surviving
        //    sibling (or at the run's end).
        for new_row in &new_rows {
            let key = norm(&new_row.path);
            if old_keys.contains(&key) {
                continue; // survivor: leave it (and its subtree) in place
            }
            let insert_at = self.insertion_point(start, child_depth, &new_rows, new_row);
            self.rows.insert(insert_at, new_row.clone());
            patches.push(TreePatch::Insert {
                index: insert_at,
                rows: vec![new_row.clone()],
            });
        }

        patches
    }

    /// Find where `new_row` should be inserted among the current direct children
    /// starting at `run_start`, preserving the disk order encoded in `new_rows`.
    /// We place it before the first *existing* direct child that comes after it
    /// in `new_rows`.
    fn insertion_point(
        &self,
        run_start: usize,
        child_depth: usize,
        new_rows: &[FlatRow],
        new_row: &FlatRow,
    ) -> usize {
        // The set of sibling paths that should come AFTER new_row per disk order.
        let new_idx = new_rows
            .iter()
            .position(|r| norm(&r.path) == norm(&new_row.path))
            .unwrap_or(0);
        let after: HashSet<String> = new_rows[new_idx + 1..]
            .iter()
            .map(|r| norm(&r.path))
            .collect();

        let mut i = run_start;
        while i < self.rows.len() && self.rows[i].depth >= child_depth {
            if self.rows[i].depth == child_depth && after.contains(&norm(&self.rows[i].path)) {
                return i;
            }
            i += 1;
        }
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), b"x").unwrap();
    }
    fn mkdir(dir: &Path, name: &str) {
        fs::create_dir(dir.join(name)).unwrap();
    }
    fn s(p: &Path) -> String {
        p.to_str().unwrap().to_string()
    }

    // 2.1 — set_root lists immediate children, dirs first, depth 0.
    #[test]
    fn set_root_lists_children() {
        let d = tempdir().unwrap();
        touch(d.path(), "b.txt");
        mkdir(d.path(), "a_dir");
        let mut t = WorkspaceTree::new();
        let rows = t.set_root(&s(d.path()));
        assert_eq!(rows.len(), 2);
        assert!(rows[0].is_dir && rows[0].name == "a_dir");
        assert!(!rows[1].is_dir && rows[1].name == "b.txt");
        assert!(rows.iter().all(|r| r.depth == 0));
    }

    // 2.2 — expand a folder inserts its children at depth+1 right after it.
    #[test]
    fn expand_inserts_children() {
        let d = tempdir().unwrap();
        mkdir(d.path(), "sub");
        touch(&d.path().join("sub"), "inner.txt");
        let mut t = WorkspaceTree::new();
        t.set_root(&s(d.path()));
        let patches = t.expand(&s(&d.path().join("sub")));

        // Update (expanded flag) + Insert(1 child).
        assert_eq!(patches.len(), 2);
        match &patches[1] {
            TreePatch::Insert { index, rows } => {
                assert_eq!(*index, 1);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].name, "inner.txt");
                assert_eq!(rows[0].depth, 1);
            }
            _ => panic!("expected Insert"),
        }
        assert_eq!(t.rows().len(), 2);
        assert!(t.rows()[0].is_expanded);
    }

    // 2.3 — collapse removes the whole subtree run.
    #[test]
    fn collapse_removes_subtree() {
        let d = tempdir().unwrap();
        mkdir(d.path(), "sub");
        let sub = d.path().join("sub");
        mkdir(&sub, "deep");
        touch(&sub, "f.txt");
        touch(&sub.join("deep"), "g.txt");

        let mut t = WorkspaceTree::new();
        t.set_root(&s(d.path()));
        t.expand(&s(&sub));
        t.expand(&s(&sub.join("deep")));
        assert_eq!(t.rows().len(), 4); // sub, deep, g.txt, f.txt

        let patches = t.collapse(&s(&sub));
        // Update + Remove(3 descendants).
        let removed = patches.iter().find_map(|p| match p {
            TreePatch::Remove { count, .. } => Some(*count),
            _ => None,
        });
        assert_eq!(removed, Some(3));
        assert_eq!(t.rows().len(), 1);
        assert!(!t.rows()[0].is_expanded);
    }

    // 2.4 — re-expanding after collapse re-lists (expanded set was cleared).
    #[test]
    fn collapse_then_expand_relists() {
        let d = tempdir().unwrap();
        mkdir(d.path(), "sub");
        touch(&d.path().join("sub"), "a.txt");
        let sub = s(&d.path().join("sub"));
        let mut t = WorkspaceTree::new();
        t.set_root(&s(d.path()));
        t.expand(&sub);
        t.collapse(&sub);
        let patches = t.expand(&sub);
        assert!(
            patches
                .iter()
                .any(|p| matches!(p, TreePatch::Insert { .. }))
        );
        assert_eq!(t.rows().len(), 2);
    }

    // 2.5 — refresh_dir on root surfaces a newly created file as one Insert.
    #[test]
    fn refresh_root_adds_new_file() {
        let d = tempdir().unwrap();
        touch(d.path(), "a.txt");
        let mut t = WorkspaceTree::new();
        t.set_root(&s(d.path()));

        touch(d.path(), "b.txt");
        let patches = t.refresh_dir(&s(d.path()));
        let inserts: usize = patches
            .iter()
            .filter(|p| matches!(p, TreePatch::Insert { .. }))
            .count();
        assert_eq!(inserts, 1);
        assert_eq!(t.rows().len(), 2);
    }

    // 2.6 — refresh_dir surfaces a deletion as exactly one Remove of count 1.
    #[test]
    fn refresh_root_removes_deleted_file() {
        let d = tempdir().unwrap();
        touch(d.path(), "a.txt");
        touch(d.path(), "b.txt");
        let mut t = WorkspaceTree::new();
        t.set_root(&s(d.path()));
        assert_eq!(t.rows().len(), 2);

        fs::remove_file(d.path().join("b.txt")).unwrap();
        let patches = t.refresh_dir(&s(d.path()));
        assert_eq!(patches.len(), 1);
        match &patches[0] {
            TreePatch::Remove { count, .. } => assert_eq!(*count, 1),
            _ => panic!("expected Remove"),
        }
        assert_eq!(t.rows().len(), 1);
    }

    // 2.7 — refresh keeps an expanded sibling's subtree intact when another
    // sibling is deleted.
    #[test]
    fn refresh_preserves_expanded_sibling_subtree() {
        let d = tempdir().unwrap();
        mkdir(d.path(), "keep");
        touch(&d.path().join("keep"), "inner.txt");
        touch(d.path(), "z_del.txt");

        let mut t = WorkspaceTree::new();
        t.set_root(&s(d.path()));
        t.expand(&s(&d.path().join("keep")));
        assert_eq!(t.rows().len(), 3); // keep, inner.txt, z_del.txt

        fs::remove_file(d.path().join("z_del.txt")).unwrap();
        t.refresh_dir(&s(d.path()));
        // keep + inner.txt survive; z_del.txt gone.
        assert_eq!(t.rows().len(), 2);
        assert!(t.rows()[0].is_expanded);
        assert_eq!(t.rows()[1].name, "inner.txt");
    }

    // 2.8 — ids are stable across collapse/expand cycles.
    #[test]
    fn ids_stable_across_cycles() {
        let d = tempdir().unwrap();
        mkdir(d.path(), "sub");
        touch(&d.path().join("sub"), "a.txt");
        let sub = s(&d.path().join("sub"));
        let mut t = WorkspaceTree::new();
        t.set_root(&s(d.path()));
        let id1 = t.expand(&sub).into_iter().find_map(|p| match p {
            TreePatch::Insert { rows, .. } => Some(rows[0].id.clone()),
            _ => None,
        });
        t.collapse(&sub);
        let id2 = t.expand(&sub).into_iter().find_map(|p| match p {
            TreePatch::Insert { rows, .. } => Some(rows[0].id.clone()),
            _ => None,
        });
        assert_eq!(id1, id2);
    }

    // 2.9 — has_children flags an empty vs non-empty folder.
    #[test]
    fn has_children_probe() {
        let d = tempdir().unwrap();
        mkdir(d.path(), "empty");
        mkdir(d.path(), "full");
        touch(&d.path().join("full"), "x");
        let mut t = WorkspaceTree::new();
        let rows = t.set_root(&s(d.path()));
        let empty = rows.iter().find(|r| r.name == "empty").unwrap();
        let full = rows.iter().find(|r| r.name == "full").unwrap();
        assert!(!empty.has_children);
        assert!(full.has_children);
    }

    // 2.10 — expand on a file or unknown path is a no-op (empty patch).
    #[test]
    fn expand_noop_cases() {
        let d = tempdir().unwrap();
        touch(d.path(), "f.txt");
        let mut t = WorkspaceTree::new();
        t.set_root(&s(d.path()));
        assert!(t.expand(&s(&d.path().join("f.txt"))).is_empty());
        assert!(t.expand("/nope").is_empty());
    }

    // 2.11 — a refresh that changes nothing yields no patches.
    #[test]
    fn refresh_no_change_no_patch() {
        let d = tempdir().unwrap();
        touch(d.path(), "a.txt");
        let mut t = WorkspaceTree::new();
        t.set_root(&s(d.path()));
        assert!(t.refresh_dir(&s(d.path())).is_empty());
    }

    // 2.12 — refresh on a deeper expanded dir inserts into the right place.
    #[test]
    fn refresh_nested_dir_inserts() {
        let d = tempdir().unwrap();
        mkdir(d.path(), "sub");
        let sub = d.path().join("sub");
        touch(&sub, "a.txt");
        let mut t = WorkspaceTree::new();
        t.set_root(&s(d.path()));
        t.expand(&s(&sub));
        assert_eq!(t.rows().len(), 2);

        touch(&sub, "b.txt");
        let patches = t.refresh_dir(&s(&sub));
        assert!(
            patches
                .iter()
                .any(|p| matches!(p, TreePatch::Insert { .. }))
        );
        assert_eq!(t.rows().len(), 3);
        // New child sits at depth 1, contiguous under sub.
        assert!(t.rows()[1].depth == 1 && t.rows()[2].depth == 1);
    }
}
