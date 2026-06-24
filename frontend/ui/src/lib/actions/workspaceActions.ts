/**
 * Workspace empty-area context actions (Requirement 4.4 / Wave 2). These fire
 * when right-clicking the blank space of the Explorer (scope `workspace_empty`),
 * acting on the workspace root. Mutations route through the same workspace-scoped
 * commands as the file-tree actions.
 */
import { registry } from '../context-menu/actionRegistry'
import type { ContextAction, ContextMenuContext } from '../context-menu/contextTypes'
import { createFile, createDir } from '../tauri/commands'
import { openFile } from '../stores/tabs'
import { refreshWorkspace } from '../stores/workspace'
import { createSession } from '../stores/terminal-sessions'
import { expandPanel } from '../stores/panels'
import { openPrompt } from '../stores/prompt-dialog'

function sep(p: string): string {
  return p.includes('\\') ? '\\' : '/'
}
function join(parent: string, name: string): string {
  const s = sep(parent)
  return parent.endsWith(s) ? parent + name : parent + s + name
}

function hasRoot(ctx: ContextMenuContext): boolean {
  return ctx.scope === 'workspace_empty' && !!ctx.workspaceRoot
}

const workspaceActions: ContextAction[] = [
  {
    id: 'workspace.newFile',
    label: 'New File…',
    icon: 'page-plus',
    group: 'create',
    order: 10,
    when: (ctx) => ctx.scope === 'workspace_empty',
    enabled: hasRoot,
    run: async (ctx) => {
      const root = ctx.workspaceRoot
      if (!root) return
      const name = await openPrompt({ title: 'New File', label: 'File name', placeholder: 'example.ts' })
      if (!name) return
      const target = join(root, name)
      try {
        await createFile(target, root)
        await refreshWorkspace()
        await openFile(target)
      } catch (e) {
        alert(`Could not create file: ${e}`)
      }
    },
  },
  {
    id: 'workspace.newFolder',
    label: 'New Folder…',
    icon: 'folder-plus',
    group: 'create',
    order: 20,
    when: (ctx) => ctx.scope === 'workspace_empty',
    enabled: hasRoot,
    run: async (ctx) => {
      const root = ctx.workspaceRoot
      if (!root) return
      const name = await openPrompt({ title: 'New Folder', label: 'Folder name', placeholder: 'src' })
      if (!name) return
      try {
        await createDir(join(root, name), root)
        await refreshWorkspace()
      } catch (e) {
        alert(`Could not create folder: ${e}`)
      }
    },
  },
  {
    id: 'workspace.openTerminalHere',
    label: 'Open Terminal Here',
    icon: 'terminal',
    group: 'navigate',
    order: 30,
    when: (ctx) => ctx.scope === 'workspace_empty',
    enabled: hasRoot,
    run: (ctx) => {
      if (!ctx.workspaceRoot) return
      expandPanel('terminal')
      createSession(ctx.workspaceRoot)
    },
  },
  {
    id: 'workspace.refreshExplorer',
    label: 'Refresh Explorer',
    icon: 'refresh',
    group: 'view',
    order: 40,
    when: (ctx) => ctx.scope === 'workspace_empty',
    run: () => void refreshWorkspace(),
  },
  {
    id: 'workspace.paste',
    label: 'Paste',
    icon: 'clipboard-check',
    group: 'clipboard',
    order: 50,
    when: (ctx) => ctx.scope === 'workspace_empty',
    // No file clipboard yet — shown disabled rather than hidden (graceful
    // degradation). Wired when copy/cut of files lands post-M9.
    enabled: () => false,
    run: () => {},
  },
]

/** Register the workspace empty-area action set (called at init). */
export function registerWorkspaceActions(): void {
  registry.registerAll(workspaceActions)
}
