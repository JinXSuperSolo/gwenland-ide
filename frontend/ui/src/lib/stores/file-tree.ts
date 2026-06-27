import { refreshDir, collapseRow, revealPath } from './tree'
import { get } from 'svelte/store'
import { workspace } from './workspace'

/**
 * File-tree signal bus (M9) — now a thin bridge to the Rust-owned flat tree
 * store (M19 Wave 2). The exported functions keep their original signatures so
 * existing callers (context-menu file actions, editor breadcrumbs) need no
 * change; each just forwards to the new patch-based tree store.
 *
 *   - `requestTreeRefresh(path)`  → reconcile that directory against disk.
 *   - `requestTreeCollapse(path)` → collapse that folder.
 *   - `requestTreeReveal(path)`   → expand ancestors so the file becomes visible.
 *
 * Root-level changes are handled by the same `refreshDir` (the engine special-
 * cases the workspace root), so there's no separate root path anymore.
 */

/** Ask the tree to re-read a directory's children (after create/delete/rename). */
export function requestTreeRefresh(path: string): void {
  void refreshDir(path)
}

/** Collapse the folder at `path` (Collapse Folder action). */
export function requestTreeCollapse(path: string): void {
  void collapseRow(path)
}

/** Expand the folders containing `path` so the file becomes visible. */
export function requestTreeReveal(path: string): void {
  const root = get(workspace).folderPath
  if (root) void revealPath(root, path)
}
