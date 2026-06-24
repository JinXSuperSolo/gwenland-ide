import { writable, get } from 'svelte/store'
import {
  openFolderDialog,
  listDirectory,
  addRecentProject,
  type DirEntry,
} from '../tauri/commands'

/**
 * The currently-open project folder and its root-level entries. Nested folder
 * children are fetched lazily on expand by FileTree itself (via listDirectory),
 * so they don't live here — this store only tracks the open root.
 */
export interface WorkspaceState {
  /** Absolute path of the open folder, or null if none is open. */
  folderPath: string | null
  /** Immediate children of `folderPath` (backend-sorted: dirs then files). */
  rootEntries: DirEntry[]
  loading: boolean
  /** Last error message (e.g. failed list), or null. Dialog-cancel is ignored. */
  error: string | null
}

const initial: WorkspaceState = {
  folderPath: null,
  rootEntries: [],
  loading: false,
  error: null,
}

export const workspace = writable<WorkspaceState>(initial)

/** The engine's cancellation message (engine/src/fs.rs FsError::DialogCancelled). */
const DIALOG_CANCELLED = 'folder selection was cancelled'

/**
 * Prompts for a folder via the native dialog, then loads its root entries into
 * the store. A cancelled dialog is a no-op (not an error). Any other failure is
 * surfaced in `error`.
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
 * Re-list the open folder's root entries (used by Refresh Explorer and after a
 * root-level file mutation). No-op when no folder is open.
 */
export async function refreshWorkspace(): Promise<void> {
  const path = get(workspace).folderPath
  if (!path) return
  try {
    const rootEntries = await listDirectory(path)
    workspace.update((s) => ({ ...s, rootEntries, error: null }))
  } catch (e) {
    workspace.update((s) => ({ ...s, error: String(e) }))
  }
}

/** Load a specific folder path (used by Open Recent + after the dialog). */
export async function openFolderPath(path: string): Promise<void> {
  workspace.update((s) => ({ ...s, folderPath: path, loading: true, error: null }))
  try {
    const rootEntries = await listDirectory(path)
    workspace.update((s) => ({ ...s, rootEntries, loading: false }))
    // Best-effort: don't fail the open if recording recents errors.
    addRecentProject(path).catch(() => {})
    // GWEN-325: any terminal already open should follow the new workspace root.
    // Dynamic import keeps the workspace store free of a Tauri/terminal cycle.
    void import('../terminal/terminal-sync').then((m) => m.autoCdSessions(path))
  } catch (e) {
    workspace.update((s) => ({ ...s, loading: false, error: String(e) }))
  }
}
