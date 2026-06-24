import { writable } from 'svelte/store'

/**
 * Lightweight signal bus for the file tree (Milestone 9). The tree's expand
 * state is decentralized (each `TreeNode` owns whether it's expanded and its
 * loaded children), so context-menu actions can't mutate it directly. Instead
 * they emit a path-targeted signal here; the matching node reacts.
 *
 *   - `refreshSignal`  → the node for `path` re-fetches its children (used after
 *      create/delete/rename/duplicate so the tree reflects disk).
 *   - `collapseSignal` → the node for `path` collapses (Collapse Folder action).
 *
 * Root-level changes are refreshed via `refreshWorkspace` in the workspace store
 * (the root entries don't live in a `TreeNode`).
 */
export interface TreeSignal {
  path: string
  /** Monotonic id so two consecutive signals for the same path both fire. */
  nonce: number
}

export const refreshSignal = writable<TreeSignal | null>(null)
export const collapseSignal = writable<TreeSignal | null>(null)
/** A reveal targets a *file* path; ancestor folder nodes expand toward it. */
export const revealSignal = writable<TreeSignal | null>(null)

let nonce = 0

/** Ask the node rendering `path` to re-read its children. */
export function requestTreeRefresh(path: string): void {
  refreshSignal.set({ path, nonce: ++nonce })
}

/** Ask the node rendering `path` to collapse. */
export function requestTreeCollapse(path: string): void {
  collapseSignal.set({ path, nonce: ++nonce })
}

/** Ask the folders containing `path` to expand so the file becomes visible
 *  ("Reveal in File Tree"). Nodes mounted by the cascade re-check the signal. */
export function requestTreeReveal(path: string): void {
  revealSignal.set({ path, nonce: ++nonce })
}
