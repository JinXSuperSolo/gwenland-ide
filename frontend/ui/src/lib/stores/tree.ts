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
 * M21 file-tree migration plan
 *
 * 1. Keep structural state Rust-owned in `WorkspaceTree` / `FlatRow[]`; keep
 *    selection, focus, and active editor state in `tree-interaction.ts`.
 * 2. Preserve the existing flat virtual list (`FileTree.svelte` +
 *    `FileTreeRow.svelte`) and tighten its overscan instead of reintroducing
 *    recursive folder components.
 * 3. Expand/collapse only through ordered splice patches; add local loading and
 *    error row metadata around the async Rust command without changing
 *    selection state.
 * 4. Filter and coalesce watcher traffic in `engine/src/fs_watch.rs`, then let
 *    the UI reconcile only structural or stale-folder changes.
 * 5. Keep incremental create/delete/rename updates scoped to the affected row
 *    range, preserving scroll position and avoiding full-tree rebuilds.
 */

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
let mutationTail: Promise<void> = Promise.resolve()
let treeGeneration = 0

const norm = (p: string) => p.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase()

/** Return refresh paths in first-seen order, ignoring duplicate spellings. Pure. */
export function uniqueTreeRefreshPaths(paths: string[]): string[] {
  const seen = new Set<string>()
  const unique: string[] = []
  for (const path of paths) {
    const key = norm(path)
    if (seen.has(key)) continue
    seen.add(key)
    unique.push(path)
  }
  return unique
}

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

function commitIfCurrent(generation: number, patches: TreePatch[]): void {
  if (generation !== treeGeneration) return
  commit(patches)
}

function patchRow(path: string, meta: Partial<FlatRow>): void {
  const target = norm(path)
  treeRows.update((rows) => {
    const idx = rows.findIndex((row) => norm(row.path) === target)
    if (idx === -1) return rows
    const next = rows.slice()
    next[idx] = { ...next[idx], ...meta }
    return next
  })
}

function enqueueTreeMutation(work: () => Promise<void>): Promise<void> {
  const run = mutationTail.then(work, work)
  mutationTail = run.catch(() => {})
  return run
}

/** Wait until queued tree patch work has reached the JS mirror. */
export async function waitForTreeIdle(): Promise<void> {
  await mutationTail
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
  void refreshDirs(dirs)
}

/**
 * Reconcile one or more dirs against Rust state, but replay the returned
 * patches against `snapshot` instead of the current optimistic mirror.
 */
export async function syncDirsFromSnapshot(snapshot: FlatRow[], dirs: string[]): Promise<void> {
  const generation = treeGeneration
  const uniqueDirs = uniqueTreeRefreshPaths(dirs)
  await enqueueTreeMutation(async () => {
    if (generation !== treeGeneration) return
    let next = snapshot
    for (const dir of uniqueDirs) {
      next = applyPatches(next, await treeRefreshDir(dir))
      if (generation !== treeGeneration) return
    }
    treeRows.set(next)
  })
}

/** Open a workspace root and seed the mirror with its root rows. */
export async function setRoot(path: string): Promise<void> {
  const generation = treeGeneration + 1
  treeGeneration = generation
  optimisticDepth = 0
  queuedRefreshes.clear()
  await enqueueTreeMutation(async () => {
    const rows = await treeSetRoot(path)
    if (generation === treeGeneration) treeRows.set(rows)
  })
}

/** Clear the mirror (workspace closed). */
export function clearTree(): void {
  treeGeneration += 1
  optimisticDepth = 0
  queuedRefreshes.clear()
  treeRows.set([])
}

/** Expand the folder at `path` and splice in its children. */
export async function expandRow(path: string): Promise<void> {
  const generation = treeGeneration
  const row = get(treeRows).find((candidate) => norm(candidate.path) === norm(path))
  if (!row || !row.is_dir || row.is_expanded || row.is_loading) return
  patchRow(path, { is_loading: true, error: null })
  await enqueueTreeMutation(async () => {
    try {
      commitIfCurrent(generation, await treeExpand(path))
      if (generation === treeGeneration) patchRow(path, { is_loading: false, error: null })
    } catch (e) {
      if (generation === treeGeneration) {
        patchRow(path, { is_loading: false, is_expanded: false, error: String(e) })
      }
    }
  })
}

/** Collapse the folder at `path` and remove its subtree. */
export async function collapseRow(path: string): Promise<void> {
  const generation = treeGeneration
  await enqueueTreeMutation(async () => {
    commitIfCurrent(generation, await treeCollapse(path))
  })
}

/** Toggle a folder row's expanded state. */
export async function toggleRow(row: FlatRow): Promise<void> {
  if (!row.is_dir || row.is_loading) return
  if (row.is_expanded) await collapseRow(row.path)
  else await expandRow(row.path)
}

/** Reconcile a directory against disk (driven by `fs:patch`). */
export async function refreshDir(path: string): Promise<void> {
  await refreshDirs([path])
}

/** Reconcile directories against disk in patch order, coalescing bursty events. */
export async function refreshDirs(paths: string[]): Promise<void> {
  const dirs = uniqueTreeRefreshPaths(paths)
  if (dirs.length === 0) return
  if (optimisticDepth > 0) {
    for (const dir of dirs) queuedRefreshes.add(dir)
    return
  }
  const generation = treeGeneration
  await enqueueTreeMutation(async () => {
    if (generation !== treeGeneration) return
    if (optimisticDepth > 0) {
      for (const dir of dirs) queuedRefreshes.add(dir)
      return
    }
    for (const dir of dirs) {
      const patches = await treeRefreshDir(dir)
      if (generation !== treeGeneration) return
      commit(patches)
    }
  })
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
