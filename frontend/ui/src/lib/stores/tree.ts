import { writable, get } from 'svelte/store'
import {
  treeSetRoot,
  treeExpand,
  treeCollapse,
  treeRefreshDir,
  type FlatRow,
  type TreePatch,
} from '../tauri/commands'

/**
 * Rust-owned flat file tree, mirrored on the JS side (M19 Wave 2).
 *
 * Rust owns the tree shape; we hold a mirror `FlatRow[]` and splice in the
 * `TreePatch` deltas every command returns. The component renders this array
 * with a virtual scroller, so only visible rows + overscan are in the DOM even
 * for a 10k-file workspace. We NEVER reconstruct the whole array from Rust after
 * the initial `setRoot` — only patches.
 */

export const treeRows = writable<FlatRow[]>([])

let optimisticDepth = 0
const queuedRefreshes = new Set<string>()

const norm = (p: string) => p.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase()

/** Apply an ordered patch list to `rows`, returning the new array. Pure. */
export function applyPatches(rows: FlatRow[], patches: TreePatch[]): FlatRow[] {
  if (patches.length === 0) return rows
  const next = rows.slice()
  for (const p of patches) {
    switch (p.kind) {
      case 'insert':
        next.splice(p.index, 0, ...p.rows)
        break
      case 'remove':
        next.splice(p.index, p.count)
        break
      case 'update':
        next[p.index] = p.row
        break
    }
  }
  return next
}

function commit(patches: TreePatch[]): void {
  if (patches.length === 0) return
  treeRows.update((rows) => applyPatches(rows, patches))
}

/** Snapshot the current flat-row mirror before an optimistic UI mutation. */
export function snapshotRows(): FlatRow[] {
  return get(treeRows)
}

/** Apply local optimistic tree patches through the same patch reconciler. */
export function applyOptimisticPatches(patches: TreePatch[]): void {
  commit(patches)
}

/** Restore a prior snapshot after an optimistic operation fails. */
export function restoreRows(rows: FlatRow[]): void {
  treeRows.set(rows)
}

/** Defer watcher-driven refreshes while an optimistic file operation is active. */
export function beginOptimisticTreeOp(): void {
  optimisticDepth += 1
}

/** Flush any refreshes that arrived while an optimistic operation was active. */
export function endOptimisticTreeOp(): void {
  optimisticDepth = Math.max(0, optimisticDepth - 1)
  if (optimisticDepth !== 0 || queuedRefreshes.size === 0) return
  const dirs = [...queuedRefreshes]
  queuedRefreshes.clear()
  for (const dir of dirs) void refreshDir(dir)
}

/**
 * Reconcile one or more dirs against Rust state, but replay the returned
 * patches against `snapshot` instead of the current optimistic mirror.
 */
export async function syncDirsFromSnapshot(snapshot: FlatRow[], dirs: string[]): Promise<void> {
  let next = snapshot
  const seen = new Set<string>()
  for (const dir of dirs) {
    if (seen.has(dir)) continue
    seen.add(dir)
    next = applyPatches(next, await treeRefreshDir(dir))
  }
  treeRows.set(next)
}

/** Open a workspace root and seed the mirror with its root rows. */
export async function setRoot(path: string): Promise<void> {
  const rows = await treeSetRoot(path)
  treeRows.set(rows)
}

/** Clear the mirror (workspace closed). */
export function clearTree(): void {
  treeRows.set([])
}

/** Expand the folder at `path` and splice in its children. */
export async function expandRow(path: string): Promise<void> {
  commit(await treeExpand(path))
}

/** Collapse the folder at `path` and remove its subtree. */
export async function collapseRow(path: string): Promise<void> {
  commit(await treeCollapse(path))
}

/** Toggle a folder row's expanded state. */
export async function toggleRow(row: FlatRow): Promise<void> {
  if (!row.is_dir) return
  if (row.is_expanded) await collapseRow(row.path)
  else await expandRow(row.path)
}

/** Reconcile a directory against disk (driven by `fs:patch`). */
export async function refreshDir(path: string): Promise<void> {
  if (optimisticDepth > 0) {
    queuedRefreshes.add(path)
    return
  }
  commit(await treeRefreshDir(path))
}

/**
 * Reveal a file: expand every ancestor folder between the root and `filePath`
 * that isn't already expanded, so the row becomes visible. Walks top-down so
 * each expand makes the next ancestor present in the mirror.
 */
export async function revealPath(rootPath: string, filePath: string): Promise<void> {
  const rootN = norm(rootPath)
  const fileN = norm(filePath)
  if (!fileN.startsWith(rootN + '/')) return

  // Ancestor directories between root (exclusive) and the file (exclusive),
  // in original (non-normalized) form, top-down.
  const rel = filePath.slice(rootPath.length).replace(/^[\\/]+/, '')
  const sep = filePath.includes('\\') ? '\\' : '/'
  const parts = rel.split(/[\\/]+/).filter(Boolean)
  parts.pop() // drop the filename
  let cur = rootPath.replace(/[\\/]+$/, '')
  for (const part of parts) {
    cur = cur + sep + part
    const row = get(treeRows).find((r) => norm(r.path) === norm(cur))
    if (row && row.is_dir && !row.is_expanded) {
      await expandRow(cur)
    }
  }
}
