/**
 * File-tree context actions (Requirement 5). Every mutation routes through a
 * workspace-scoped Tauri command (Strict Rule 2); Delete confirms first (Rule
 * 3); nothing assumes `ctx.path`/`ctx.workspaceRoot` without gating on it
 * (Rule 5). After a mutation the affected directory is refreshed so the tree
 * reflects disk.
 *
 * Order values increase across groups (create → edit → clipboard → navigate →
 * view) so the registry lays the groups out in that sequence with separators.
 */
import { registry } from '../context-menu/actionRegistry'
import type { ContextAction, ContextMenuContext } from '../context-menu/contextTypes'
import {
  createFile,
  createDir,
  renamePath,
  deletePath,
  duplicatePath,
  revealInExplorer,
  moveToTrash,
  markProtectedPath,
  readFile,
} from '../tauri/commands'
import { openFile, openFileToSide } from '../stores/tabs'
import { refreshWorkspace } from '../stores/workspace'
import { requestTreeRefresh, requestTreeCollapse } from '../stores/file-tree'
import { createSession } from '../stores/terminal-sessions'
import { expandPanel } from '../stores/panels'
import { openTreeInput } from '../stores/tree-input'
import { openLocalHistory } from '../stores/local-history'
import { explainFile, refactorFile } from '../stores/ai-actions'
import { openSimpleDiff } from '../stores/simple-diff'
import { toast } from '../stores/toast'

// --- Path helpers (OS-separator aware: engine paths use the native separator) ---

function sep(p: string): string {
  return p.includes('\\') ? '\\' : '/'
}
function basename(p: string): string {
  return p.split(/[\\/]/).filter(Boolean).pop() || p
}
function dirname(p: string): string {
  const idx = Math.max(p.lastIndexOf('\\'), p.lastIndexOf('/'))
  return idx <= 0 ? p : p.slice(0, idx)
}
function join(parent: string, name: string): string {
  const s = sep(parent)
  return parent.endsWith(s) ? parent + name : parent + s + name
}
function samePath(a: string, b: string): boolean {
  const norm = (p: string) => p.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase()
  return norm(a) === norm(b)
}
function relativeTo(root: string, p: string): string {
  if (root && p.toLowerCase().startsWith(root.toLowerCase())) {
    const rel = p.slice(root.length).replace(/^[\\/]+/, '')
    return rel || basename(p)
  }
  return basename(p)
}

let compareAnchor: string | null = null

/** The directory a file-tree action targets: the folder itself, else its parent. */
function targetDir(ctx: ContextMenuContext): string | null {
  if (!ctx.path) return null
  return ctx.isDirectory ? ctx.path : dirname(ctx.path)
}

/** Refresh the folder a mutation touched — root via the workspace store, nested
 *  folders via a path-targeted tree signal. */
function refreshDir(dir: string, workspaceRoot?: string): void {
  if (workspaceRoot && samePath(dir, workspaceRoot)) void refreshWorkspace()
  else requestTreeRefresh(dir)
}

/** Common guard: file-tree scope with a usable path + workspace root. */
function inWorkspace(ctx: ContextMenuContext): boolean {
  return ctx.scope === 'file_tree' && !!ctx.path && !!ctx.workspaceRoot
}

async function moveContextPathToTrash(ctx: ContextMenuContext): Promise<void> {
  if (!ctx.path || !ctx.workspaceRoot) return
  if (!confirm(`Move "${basename(ctx.path)}" to Trash?`)) return
  const name = basename(ctx.path)
  const dir = dirname(ctx.path)
  // Yield to the render thread before the blocking operation begins, keeping
  // the UI responsive. A toast shows on completion so the user knows it's done.
  await new Promise<void>((resolve) => setTimeout(resolve, 0))
  try {
    await moveToTrash(ctx.path, ctx.workspaceRoot)
    refreshDir(dir, ctx.workspaceRoot)
    toast(`"${name}" moved to Trash`, 'success')
  } catch (e) {
    alert(`Could not move to trash: ${e}`)
  }
}

async function deleteContextPathPermanently(ctx: ContextMenuContext): Promise<void> {
  if (!ctx.path || !ctx.workspaceRoot) return
  if (!confirm(`Are you sure? This cannot be undone.\n\nDelete "${basename(ctx.path)}" permanently?`)) {
    return
  }
  const name = basename(ctx.path)
  const dir = dirname(ctx.path)
  // Yield to the render thread before the blocking operation begins.
  await new Promise<void>((resolve) => setTimeout(resolve, 0))
  try {
    await deletePath(ctx.path, ctx.workspaceRoot)
    refreshDir(dir, ctx.workspaceRoot)
    toast(`"${name}" deleted permanently`, 'success')
  } catch (e) {
    alert(`Could not delete permanently: ${e}`)
  }
}

const fileTreeActions: ContextAction[] = [
  // ── create ──────────────────────────────────────────────────────────────
  {
    id: 'file.newFile',
    label: 'New File…',
    icon: 'page-plus',
    group: 'create',
    order: 10,
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: inWorkspace,
    run: async (ctx) => {
      const dir = targetDir(ctx)
      if (!dir || !ctx.workspaceRoot) return
      const name = await openTreeInput({ kind: 'file', targetDir: dir, icon: 'page' })
      if (!name) return
      const target = join(dir, name)
      try {
        await createFile(target, ctx.workspaceRoot)
        refreshDir(dir, ctx.workspaceRoot)
        await openFile(target)
      } catch (e) {
        alert(`Could not create file: ${e}`)
      }
    },
  },
  {
    id: 'file.newFolder',
    label: 'New Folder…',
    icon: 'folder-plus',
    group: 'create',
    order: 20,
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: inWorkspace,
    run: async (ctx) => {
      const dir = targetDir(ctx)
      if (!dir || !ctx.workspaceRoot) return
      const name = await openTreeInput({ kind: 'folder', targetDir: dir, icon: 'folder' })
      if (!name) return
      try {
        await createDir(join(dir, name), ctx.workspaceRoot)
        refreshDir(dir, ctx.workspaceRoot)
      } catch (e) {
        alert(`Could not create folder: ${e}`)
      }
    },
  },

  // ── edit ────────────────────────────────────────────────────────────────
  {
    id: 'file.rename',
    label: 'Rename',
    icon: 'edit-pencil',
    group: 'edit',
    order: 30,
    shortcut: 'F2',
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: inWorkspace,
    run: async (ctx) => {
      if (!ctx.path || !ctx.workspaceRoot) return
      const current = basename(ctx.path)
      const dir = dirname(ctx.path)
      const name = await openTreeInput({
        kind: 'rename',
        targetDir: dir,
        initialValue: current,
        icon: ctx.isDirectory ? 'folder' : 'page',
      })
      if (!name || name === current) return
      try {
        // NOTE: an already-open tab keeps its old path until reopened; updating
        // live tabs on rename is out of M9 scope.
        await renamePath(ctx.path, join(dir, name), ctx.workspaceRoot)
        refreshDir(dir, ctx.workspaceRoot)
      } catch (e) {
        alert(`Could not rename: ${e}`)
      }
    },
  },
  {
    id: 'file.delete',
    label: 'Move to Trash',
    icon: 'bin',
    group: 'edit',
    order: 40,
    shortcut: 'Del',
    danger: true,
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: inWorkspace,
    run: moveContextPathToTrash,
  },
  {
    id: 'file.deletePermanently',
    label: 'Delete Permanently',
    icon: 'bin',
    group: 'edit',
    order: 45,
    shortcut: 'Shift+Del',
    danger: true,
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: inWorkspace,
    run: deleteContextPathPermanently,
  },
  {
    id: 'file.duplicate',
    label: 'Duplicate',
    icon: 'copy',
    group: 'edit',
    order: 50,
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: inWorkspace,
    run: async (ctx) => {
      if (!ctx.path || !ctx.workspaceRoot) return
      const dir = dirname(ctx.path)
      try {
        await duplicatePath(ctx.path, ctx.workspaceRoot)
        refreshDir(dir, ctx.workspaceRoot)
      } catch (e) {
        alert(`Could not duplicate: ${e}`)
      }
    },
  },

  // ── clipboard ─────────────────────────────────────────────────────────────
  {
    id: 'file.copyPath',
    label: 'Copy Path',
    icon: 'copy',
    group: 'clipboard',
    order: 60,
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) void navigator.clipboard.writeText(ctx.path)
    },
  },
  {
    id: 'file.copyRelativePath',
    label: 'Copy Relative Path',
    icon: 'copy',
    group: 'clipboard',
    order: 70,
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) void navigator.clipboard.writeText(relativeTo(ctx.workspaceRoot ?? '', ctx.path))
    },
  },

  // ── navigate ──────────────────────────────────────────────────────────────
  {
    id: 'file.revealInExplorer',
    label: 'Reveal in Explorer',
    icon: 'folder',
    group: 'navigate',
    order: 80,
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: inWorkspace,
    run: async (ctx) => {
      if (!ctx.path || !ctx.workspaceRoot) return
      try {
        await revealInExplorer(ctx.path, ctx.workspaceRoot)
      } catch (e) {
        console.error('reveal in explorer failed:', e)
      }
    },
  },
  {
    id: 'file.openInTerminal',
    label: 'Open in Terminal',
    icon: 'terminal',
    group: 'navigate',
    order: 90,
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: (ctx) => !!targetDir(ctx),
    run: (ctx) => {
      const dir = targetDir(ctx)
      if (!dir) return
      expandPanel('terminal')
      createSession(dir)
    },
  },
  {
    id: 'file.openToSide',
    label: 'Open to Side',
    icon: 'page',
    group: 'navigate',
    order: 100,
    when: (ctx) => ctx.scope === 'file_tree' && ctx.isDirectory !== true,
    enabled: (ctx) => !!ctx.path,
    run: async (ctx) => {
      if (ctx.path) await openFileToSide(ctx.path)
    },
  },
  {
    id: 'file.compareWithSelected',
    label: compareAnchor ? 'Compare With Selected' : 'Select for Compare',
    icon: 'code',
    group: 'navigate',
    order: 105,
    when: (ctx) => ctx.scope === 'file_tree' && ctx.isDirectory !== true,
    enabled: (ctx) => !!ctx.path,
    run: async (ctx) => {
      if (!ctx.path) return
      if (!compareAnchor || compareAnchor === ctx.path) {
        compareAnchor = ctx.path
        alert(`Selected for compare: ${basename(ctx.path)}`)
        return
      }
      const left = await readFile(compareAnchor).catch(() => '')
      const right = await readFile(ctx.path).catch(() => '')
      openSimpleDiff({
        title: 'Compare With Selected',
        leftLabel: compareAnchor,
        rightLabel: ctx.path,
        left,
        right,
      })
      compareAnchor = null
    },
  },
  {
    id: 'file.showLocalHistory',
    label: 'Show Local History',
    icon: 'clock-rotate-right',
    group: 'navigate',
    order: 106,
    when: (ctx) => ctx.scope === 'file_tree' && ctx.isDirectory !== true,
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) void openLocalHistory(ctx.path)
    },
  },
  {
    id: 'file.markProtected',
    label: 'Mark as Protected',
    icon: 'warning-triangle',
    group: 'safety',
    order: 107,
    when: (ctx) => ctx.scope === 'file_tree',
    enabled: inWorkspace,
    run: async (ctx) => {
      if (!ctx.path || !ctx.workspaceRoot) return
      await markProtectedPath(ctx.path, ctx.workspaceRoot)
    },
  },
  {
    id: 'file.aiExplain',
    label: 'AI: Explain File',
    icon: 'sparks',
    group: 'ai',
    order: 108,
    when: (ctx) => ctx.scope === 'file_tree' && ctx.isDirectory !== true,
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) void explainFile(ctx.path)
    },
  },
  {
    id: 'file.aiRefactor',
    label: 'AI: Refactor File',
    icon: 'magic-wand',
    group: 'ai',
    order: 109,
    when: (ctx) => ctx.scope === 'file_tree' && ctx.isDirectory !== true,
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) void refactorFile(ctx.path)
    },
  },

  // ── view ──────────────────────────────────────────────────────────────────
  {
    id: 'file.collapseFolder',
    label: 'Collapse Folder',
    icon: 'nav-arrow-right',
    group: 'view',
    order: 110,
    when: (ctx) => ctx.scope === 'file_tree' && ctx.isDirectory === true,
    run: (ctx) => {
      if (ctx.path) requestTreeCollapse(ctx.path)
    },
  },
  {
    id: 'file.refresh',
    label: 'Refresh',
    icon: 'refresh',
    group: 'view',
    order: 120,
    when: (ctx) => ctx.scope === 'file_tree',
    run: (ctx) => {
      const dir = targetDir(ctx)
      if (dir) refreshDir(dir, ctx.workspaceRoot)
    },
  },
]

/** Register the file-tree action set into the shared registry (called at init). */
export function registerFileActions(): void {
  registry.registerAll(fileTreeActions)
}
