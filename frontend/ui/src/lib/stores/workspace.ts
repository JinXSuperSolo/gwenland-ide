import { writable, get } from 'svelte/store'
import { openFolderDialog, addRecentProject, fsWatchDir } from '../tauri/commands'
import { setRoot, clearTree, refreshDir } from './tree'

/**
 * The currently-open project folder. The tree's row data lives in the Rust-owned
 * flat-tree store (`./tree`, M19 Wave 2); this store only tracks which folder is
 * open plus load/error state.
 */
export interface WorkspaceState {
  /** Absolute path of the open folder, or null if none is open. */
  folderPath: string | null
  loading: boolean
  /** Last error message (e.g. failed list), or null. Dialog-cancel is ignored. */
  error: string | null
}

const initial: WorkspaceState = {
  folderPath: null,
  loading: false,
  error: null,
}

export const workspace = writable<WorkspaceState>(initial)

/** The engine's cancellation message (engine/src/fs.rs FsError::DialogCancelled). */
const DIALOG_CANCELLED = 'folder selection was cancelled'

/**
 * Prompts for a folder via the native dialog, then opens it. A cancelled dialog
 * is a no-op (not an error). Any other failure is surfaced in `error`.
 */
export async function openFolder(): Promise<void> {
  let path: string
  try {
    path = await openFolderDialog()
  } catch (e) {
    // User dismissed the picker — leave current state untouched.
    if (String(e).includes(DIALOG_CANCELLED)) return
    workspace.update((s) => ({ ...s, error: String(e) }))
    return
  }

  await openFolderPath(path)
}

/**
 * Re-read the open folder's root rows (Refresh Explorer, and after a root-level
 * file mutation). Reconciles via tree patches. No-op when no folder is open.
 */
export async function refreshWorkspace(): Promise<void> {
  const path = get(workspace).folderPath
  if (!path) return
  try {
    await refreshDir(path)
    workspace.update((s) => ({ ...s, error: null }))
  } catch (e) {
    workspace.update((s) => ({ ...s, error: String(e) }))
  }
}

/** Load a specific folder path (used by Open Recent + after the dialog). */
export async function openFolderPath(path: string): Promise<void> {
  workspace.update((s) => ({ ...s, folderPath: path, loading: true, error: null }))
  try {
    // Seed the Rust-owned tree with the root rows (the initial render).
    await setRoot(path)
    workspace.update((s) => ({ ...s, loading: false }))
    // M19 Wave 1: watch the workspace root so root-level changes refresh the
    // tree. Nested folders register themselves on expand (FileTreeRow).
    void fsWatchDir(path)
    // Best-effort: don't fail the open if recording recents errors.
    addRecentProject(path).catch(() => {})
    // GWEN-325: any terminal already open should follow the new workspace root.
    // Dynamic import keeps the workspace store free of a Tauri/terminal cycle.
    void import('../terminal/terminal-sync').then((m) => m.autoCdSessions(path))
  } catch (e) {
    clearTree()
    workspace.update((s) => ({ ...s, loading: false, error: String(e) }))
  }
}
