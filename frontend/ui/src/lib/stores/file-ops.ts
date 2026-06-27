import {
  createDir,
  createFile,
  deletePath,
  moveToWorkspaceTrash,
  renamePath,
  type TrashRecord,
  type FlatRow,
} from '../tauri/commands'
import type { TreePatch } from '../tauri/commands'
import { refreshGit } from './git'
import { toast } from './toast'
import {
  applyOptimisticPatches,
  beginOptimisticTreeOp,
  endOptimisticTreeOp,
  restoreRows,
  snapshotRows,
  syncDirsFromSnapshot,
  waitForTreeIdle,
} from './tree'
import {
  buildOptimisticCreatePatches,
  buildOptimisticMovePatches,
  buildOptimisticRemovePatches,
  dirname,
  joinPath,
  pathSep,
  samePath,
} from './file-op-patches'

const UNDO_LIMIT = 10

type UndoEntry = {
  label: string
  run: () => Promise<boolean>
}

const undoHistory: UndoEntry[] = []

function pushUndo(entry: UndoEntry): void {
  undoHistory.unshift(entry)
  if (undoHistory.length > UNDO_LIMIT) undoHistory.length = UNDO_LIMIT
}

export function fileOpUndoDepth(): number {
  return undoHistory.length
}

function absoluteFromWorkspace(root: string, relative: string): string {
  const sep = pathSep(root)
  return joinPath(root, relative.split(/[\\/]/).filter(Boolean).join(sep))
}

async function runOptimistic<T>(opts: {
  patches: (snapshot: FlatRow[]) => TreePatch[]
  syncDirs: string[]
  execute: () => Promise<T>
  failure: string
  undo?: (value: T) => UndoEntry | null
  success?: string
}): Promise<T | null> {
  await waitForTreeIdle()
  const snapshot = snapshotRows()
  beginOptimisticTreeOp()
  applyOptimisticPatches(opts.patches(snapshot))
  try {
    const value = await opts.execute()
    await syncDirsFromSnapshot(snapshot, opts.syncDirs)
    const undo = opts.undo?.(value)
    if (undo) pushUndo(undo)
    if (opts.success) toast(opts.success, 'success')
    void refreshGit()
    return value
  } catch (e) {
    restoreRows(snapshot)
    toast(`${opts.failure}: ${String(e)}`, 'error')
    return null
  } finally {
    endOptimisticTreeOp()
  }
}

export async function optimisticCreateFile(path: string, workspaceRoot: string): Promise<boolean> {
  const ok = await runOptimistic({
    patches: (snapshot) => buildOptimisticCreatePatches(snapshot, workspaceRoot, path, false),
    syncDirs: [dirname(path)],
    execute: () => createFile(path, workspaceRoot),
    failure: 'Could not create file',
    undo: () => ({
      label: 'create file',
      run: () => optimisticDeletePath(path, workspaceRoot, false),
    }),
  })
  return ok !== null
}

export async function optimisticCreateDir(path: string, workspaceRoot: string): Promise<boolean> {
  const ok = await runOptimistic({
    patches: (snapshot) => buildOptimisticCreatePatches(snapshot, workspaceRoot, path, true),
    syncDirs: [dirname(path)],
    execute: () => createDir(path, workspaceRoot),
    failure: 'Could not create folder',
    undo: () => ({
      label: 'create folder',
      run: () => optimisticDeletePath(path, workspaceRoot, false),
    }),
  })
  return ok !== null
}

export async function optimisticMovePath(
  oldPath: string,
  newPath: string,
  workspaceRoot: string,
  recordUndo = true,
): Promise<boolean> {
  if (samePath(oldPath, newPath)) return true
  const ok = await runOptimistic({
    patches: (snapshot) => buildOptimisticMovePatches(snapshot, workspaceRoot, oldPath, newPath),
    syncDirs: [dirname(oldPath), dirname(newPath)],
    execute: () => renamePath(oldPath, newPath, workspaceRoot),
    failure: 'Could not move path',
    undo: recordUndo
      ? () => ({
          label: 'move',
          run: () => optimisticMovePath(newPath, oldPath, workspaceRoot, false),
        })
      : undefined,
  })
  return ok !== null
}

export const optimisticRenamePath = optimisticMovePath

export async function optimisticDeletePath(
  path: string,
  workspaceRoot: string,
  recordUndo = true,
): Promise<boolean> {
  let deletedIsDir = false
  const ok = await runOptimistic<TrashRecord>({
    patches: (snapshot) => {
      deletedIsDir = snapshot.find((row) => samePath(row.path, path))?.is_dir ?? false
      return buildOptimisticRemovePatches(snapshot, workspaceRoot, path)
    },
    syncDirs: [dirname(path)],
    execute: () => moveToWorkspaceTrash(path, workspaceRoot),
    failure: 'Could not move to Trash',
    undo: recordUndo
      ? (record) => ({
          label: 'delete',
          run: () =>
            optimisticRestoreTrashRecord(
              record,
              absoluteFromWorkspace(workspaceRoot, record.original_path),
              workspaceRoot,
              deletedIsDir,
            ),
        })
      : undefined,
  })
  return ok !== null
}

async function optimisticRestoreTrashRecord(
  record: TrashRecord,
  restorePath: string,
  workspaceRoot: string,
  isDir: boolean,
): Promise<boolean> {
  const ok = await runOptimistic({
    patches: (snapshot) => buildOptimisticCreatePatches(snapshot, workspaceRoot, restorePath, isDir),
    syncDirs: [dirname(restorePath), dirname(record.trash_path)],
    execute: () => renamePath(record.trash_path, restorePath, workspaceRoot),
    failure: 'Could not undo delete',
  })
  return ok !== null
}

export async function optimisticPermanentDeletePath(
  path: string,
  workspaceRoot: string,
): Promise<boolean> {
  const ok = await runOptimistic({
    patches: (snapshot) => buildOptimisticRemovePatches(snapshot, workspaceRoot, path),
    syncDirs: [dirname(path)],
    execute: () => deletePath(path, workspaceRoot),
    failure: 'Could not delete permanently',
  })
  return ok !== null
}

export async function undoLastFileOp(): Promise<boolean> {
  const entry = undoHistory.shift()
  if (!entry) {
    toast('Nothing to undo', 'info')
    return false
  }
  const ok = await entry.run()
  if (ok) {
    toast(`Undid ${entry.label}`, 'success')
    return true
  }
  undoHistory.unshift(entry)
  return false
}
