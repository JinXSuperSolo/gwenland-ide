import { get } from 'svelte/store'
import { onFsPatch, fsWatchClear, type FsPatch } from '../tauri/commands'
import { workspace } from './workspace'
import { refreshDir } from './tree'
import { refreshGit } from './git'

/**
 * File-watcher → UI bridge (M19 Wave 1, GWEN-376).
 *
 * The engine emits one coalesced `fs:patch` event per poll cycle, each carrying
 * every changed directory's batch. Each patched directory is reconciled through
 * the Rust-owned tree store via `refreshDir`, which returns minimal add/remove
 * tree patches. Folder watch registration/unregistration lives in
 * `FileTreeRow` (watch while expanded, unwatch on collapse/unmount).
 *
 * Git decorations are refreshed at most once per 500ms after the last patch, so
 * a burst (e.g. `npm install`) triggers a single git rescan, not one per file.
 */

const GIT_DEBOUNCE_MS = 500
let gitTimer: ReturnType<typeof setTimeout> | null = null

/** Schedule a debounced git refresh — coalesces a burst into one rescan. */
function scheduleGitRefresh(): void {
  if (gitTimer) clearTimeout(gitTimer)
  gitTimer = setTimeout(() => {
    gitTimer = null
    if (get(workspace).folderPath) void refreshGit()
  }, GIT_DEBOUNCE_MS)
}

/** Reconcile each changed directory through the tree store, then refresh git. */
function applyFsPatches(patches: FsPatch[]): void {
  if (patches.length === 0) return
  for (const patch of patches) {
    // `refreshDir` reconciles via tree patches; the engine special-cases the
    // workspace root, so root and nested dirs go through the same path.
    void refreshDir(patch.dir)
  }
  scheduleGitRefresh()
}

let inited = false
let unlisten: (() => void) | null = null
let lastRoot: string | null = null

/**
 * Wire the file watcher. Subscribes to `fs:patch` once and clears the engine's
 * watch set whenever the workspace closes/switches (folder nodes re-register
 * their watches as they mount/expand under the new root). Idempotent.
 */
export function initFsWatch(): void {
  if (inited) return
  inited = true

  onFsPatch(applyFsPatches).then((fn) => {
    unlisten = fn
  })

  // On workspace close/switch, drop all engine-side watches. Newly mounted tree
  // nodes register their own folders again under the new root.
  workspace.subscribe((s) => {
    if (s.folderPath !== lastRoot) {
      lastRoot = s.folderPath
      void fsWatchClear()
    }
  })
}

/** Tear down the listener (hot-reload / test cleanup). */
export function disposeFsWatch(): void {
  if (unlisten) {
    unlisten()
    unlisten = null
  }
  if (gitTimer) {
    clearTimeout(gitTimer)
    gitTimer = null
  }
  inited = false
}
