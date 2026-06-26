import { get, writable } from 'svelte/store'
import {
  editorAddCursorAbove,
  editorAddCursorBelow,
  editorCopy,
  editorCopyLineDown,
  editorCopyLineUp,
  editorCut,
  editorDeleteLine,
  editorFind,
  editorFindWorkspace,
  editorFormatDocument,
  editorGoToDefinition,
  editorGoToLine,
  editorMoveLineDown,
  editorMoveLineUp,
  editorPaste,
  editorRedo,
  editorRenameSymbol,
  editorReplace,
  editorSelectAll,
  editorSelectAllMatches,
  editorSelectNextMatch,
  editorToggleComment,
  editorUndo,
} from '../editor/active-editor'
import { getTerminalHandle } from '../terminal/terminal-registry'
import { git, refreshGit } from '../stores/git'
import { openPrompt } from '../stores/prompt-dialog'
import { aiChat } from '../stores/ai-chat'
import { collapsePanel, expandPanel, panels, togglePanel } from '../stores/panels'
import { showSidebarView } from '../stores/sidebar'
import {
  closeActiveTab,
  closeAllTabs,
  closeSavedTabs,
  newUntitledFile,
  openPreview,
  saveActiveTab,
  saveActiveTabAs,
  showOpenedEditors,
  toggleLockActiveGroup,
  toggleMaximizeActiveGroup,
} from '../stores/tabs'
import { terminalSessions, createSession, removeSession } from '../stores/terminal-sessions'
import { openFolder, workspace } from '../stores/workspace'
import { openPalette, openSettings } from '../stores/ui'
import {
  clearLocalHistory,
  createManualHistorySnapshot,
  openLocalHistory,
} from '../stores/local-history'
import {
  explainSelection,
  fixSelection,
  generateTestsForSelection,
} from '../stores/ai-actions'
import {
  gitCheckout,
  gitCreateBranch,
  gitDeleteBranch,
  gitListBranches,
  terminalKill,
} from '../tauri/commands'

export type Command = {
  id: string
  title: string
  category: string
  defaultKeybinding?: string
  when?: string
  handler: () => void | Promise<void>
}

export type MenuItem = {
  label?: string
  commandId?: string
  shortcut?: string
  disabled?: boolean
  type?: 'divider'
  children?: 'recent'
}

const byId = new Map<string, Command>()
export const commands = writable<Command[]>([])

function register(command: Command): void {
  byId.set(command.id, command)
}

export function registerCommand(command: Command): void {
  register(command)
  commands.set([...byId.values()])
}

function noop(name: string): void {
  console.info(`[GwenLand] "${name}" is wired and waiting for its backend surface.`)
}

function ensureAiOpen(): void {
  aiChat.update((s) => ({ ...s, isOpen: true }))
}

function toggleTerminalPanel(): void {
  if (!get(workspace).folderPath) return
  const opening = get(panels).terminal.collapsed
  togglePanel('terminal')
  if (opening && get(terminalSessions).sessions.length === 0) createSession()
}

function newTerminal(): void {
  if (!get(workspace).folderPath) return
  expandPanel('terminal')
  createSession()
}

function killActiveTerminal(): void {
  const activeKey = get(terminalSessions).activeKey
  if (!activeKey) return
  const ptyId = removeSession(activeKey)
  if (ptyId) void terminalKill(ptyId).catch(() => {})
  if (get(terminalSessions).sessions.length === 0) collapsePanel('terminal')
}

function clearActiveTerminal(): void {
  getTerminalHandle(get(terminalSessions).activeKey ?? undefined)?.clear()
}

function focusTerminal(): void {
  getTerminalHandle(get(terminalSessions).activeKey ?? undefined)?.focus()
}

function normalizePreviewInput(input: string): { url: string; port: number } | null {
  const raw = input.trim()
  if (!raw) return null
  const withScheme = /^\d+$/.test(raw)
    ? `http://localhost:${raw}`
    : /^https?:\/\//i.test(raw)
      ? raw
      : `http://${raw}`
  try {
    const url = new URL(withScheme)
    const port = Number(url.port || (url.protocol === 'https:' ? 443 : 80))
    return Number.isInteger(port) && port > 0 ? { url: url.toString(), port } : null
  } catch {
    return null
  }
}

function repoRoot(): string | null {
  const root = get(workspace).folderPath
  return root && get(git).isRepo ? root : null
}

async function checkoutBranch(): Promise<void> {
  const root = repoRoot()
  if (!root) return
  let branches: string[] = []
  try {
    branches = await gitListBranches(root)
  } catch {
    branches = []
  }
  const name = await openPrompt({
    title: 'Checkout Branch',
    label: branches.length ? `Branch name (${branches.join(', ')})` : 'Branch name',
    placeholder: branches[0] ?? 'main',
  })
  if (!name) return
  try {
    await gitCheckout(root, name)
    await refreshGit()
  } catch (e) {
    alert(`Checkout failed: ${e}`)
  }
}

async function createBranch(): Promise<void> {
  const root = repoRoot()
  if (!root) return
  const name = await openPrompt({
    title: 'Create Branch',
    label: 'New branch name',
    placeholder: 'my-feature',
  })
  if (!name) return
  try {
    await gitCreateBranch(root, name)
    await refreshGit()
  } catch (e) {
    alert(`Create branch failed: ${e}`)
  }
}

async function deleteBranch(): Promise<void> {
  const root = repoRoot()
  if (!root) return
  const current = get(git).branch
  let branches: string[] = []
  try {
    branches = (await gitListBranches(root)).filter((branch) => branch !== current)
  } catch {
    branches = []
  }
  if (branches.length === 0) {
    alert('No other branches to delete.')
    return
  }
  const name = await openPrompt({
    title: 'Delete Branch',
    label: `Branch to delete (${branches.join(', ')})`,
    placeholder: branches[0],
  })
  if (!name || name === current) return
  if (!confirm(`Delete branch "${name}"? This cannot be undone.`)) return
  try {
    await gitDeleteBranch(root, name)
    await refreshGit()
  } catch (e) {
    alert(`Delete branch failed: ${e}`)
  }
}

async function openWebPreview(): Promise<void> {
  const terminal = get(terminalSessions)
  if (terminal.detectedPort) {
    openPreview({
      kind: 'dev-server',
      url: terminal.detectedUrl ?? `http://localhost:${terminal.detectedPort}`,
      port: terminal.detectedPort,
    })
    return
  }

  const input = await openPrompt({
    title: 'Open Web Preview',
    label: 'URL or localhost port',
    placeholder: '5173',
    confirmLabel: 'Open',
  })
  if (!input) return
  const preview = normalizePreviewInput(input)
  if (preview) openPreview({ kind: 'dev-server', ...preview })
}

const COMMANDS: Command[] = [
  {
    id: 'workbench.action.showCommands',
    title: 'Command Palette',
    category: 'View',
    defaultKeybinding: 'Ctrl+Shift+P / F1',
    handler: openPalette,
  },
  {
    id: 'workbench.action.quickOpen',
    title: 'Quick Open',
    category: 'File',
    defaultKeybinding: 'Ctrl+P',
    handler: openPalette,
  },
  {
    id: 'palette.open',
    title: 'Command Palette',
    category: 'View',
    defaultKeybinding: 'Ctrl+Shift+P / F1',
    handler: openPalette,
  },
  {
    id: 'file.new',
    title: 'New File',
    category: 'File',
    defaultKeybinding: 'Ctrl+N',
    handler: newUntitledFile,
  },
  {
    id: 'file.openFolder',
    title: 'Open Folder',
    category: 'File',
    handler: () => void openFolder(),
  },
  {
    id: 'file.save',
    title: 'Save',
    category: 'File',
    defaultKeybinding: 'Ctrl+S',
    handler: () => void saveActiveTab(),
  },
  {
    id: 'file.saveAs',
    title: 'Save As',
    category: 'File',
    defaultKeybinding: 'Ctrl+Shift+S',
    handler: () => void saveActiveTabAs(),
  },
  {
    id: 'fileHistory.createSnapshot',
    title: 'Local History: Create Snapshot',
    category: 'File',
    handler: () => void createManualHistorySnapshot(),
  },
  {
    id: 'fileHistory.show',
    title: 'Local History: Show Current File',
    category: 'File',
    defaultKeybinding: 'Ctrl+Alt+H',
    handler: () => void openLocalHistory(),
  },
  {
    id: 'fileHistory.clearHistory',
    title: 'Local History: Clear Current File',
    category: 'File',
    handler: () => void clearLocalHistory(),
  },
  {
    id: 'tab.close',
    title: 'Close Tab',
    category: 'File',
    handler: closeActiveTab,
  },
  {
    id: 'tab.closeAll',
    title: 'Close All',
    category: 'File',
    handler: closeAllTabs,
  },
  {
    id: 'editor.closeAll',
    title: 'Close All in Group',
    category: 'View',
    defaultKeybinding: 'Ctrl+K W',
    handler: closeAllTabs,
  },
  {
    id: 'editor.closeSaved',
    title: 'Close Saved in Group',
    category: 'View',
    handler: closeSavedTabs,
  },
  {
    id: 'edit.undo',
    title: 'Undo',
    category: 'Edit',
    handler: editorUndo,
  },
  {
    id: 'edit.redo',
    title: 'Redo',
    category: 'Edit',
    handler: editorRedo,
  },
  {
    id: 'edit.cut',
    title: 'Cut',
    category: 'Edit',
    handler: () => void editorCut(),
  },
  {
    id: 'edit.copy',
    title: 'Copy',
    category: 'Edit',
    handler: () => void editorCopy(),
  },
  {
    id: 'edit.paste',
    title: 'Paste',
    category: 'Edit',
    handler: () => void editorPaste(),
  },
  {
    id: 'search.inFile',
    title: 'Find in File',
    category: 'Search',
    defaultKeybinding: 'Ctrl+F',
    handler: editorFind,
  },
  {
    id: 'search.replaceInFile',
    title: 'Replace in File',
    category: 'Search',
    defaultKeybinding: 'Ctrl+H',
    handler: editorReplace,
  },
  {
    id: 'search.inWorkspace',
    title: 'Find in Workspace',
    category: 'Search',
    defaultKeybinding: 'Ctrl+Shift+F',
    handler: editorFindWorkspace,
  },
  {
    id: 'editor.toggleComment',
    title: 'Toggle Comment',
    category: 'Edit',
    defaultKeybinding: 'Ctrl+/',
    handler: editorToggleComment,
  },
  {
    id: 'editor.formatDocument',
    title: 'Format Document',
    category: 'Edit',
    defaultKeybinding: 'Shift+Alt+F',
    handler: editorFormatDocument,
  },
  {
    id: 'editor.moveLineUp',
    title: 'Move Line Up',
    category: 'Edit',
    defaultKeybinding: 'Alt+Up',
    handler: editorMoveLineUp,
  },
  {
    id: 'editor.moveLineDown',
    title: 'Move Line Down',
    category: 'Edit',
    defaultKeybinding: 'Alt+Down',
    handler: editorMoveLineDown,
  },
  {
    id: 'editor.copyLineUp',
    title: 'Copy Line Up',
    category: 'Edit',
    defaultKeybinding: 'Shift+Alt+Up',
    handler: editorCopyLineUp,
  },
  {
    id: 'editor.copyLineDown',
    title: 'Copy Line Down',
    category: 'Edit',
    defaultKeybinding: 'Shift+Alt+Down',
    handler: editorCopyLineDown,
  },
  {
    id: 'editor.deleteLine',
    title: 'Delete Line',
    category: 'Edit',
    defaultKeybinding: 'Ctrl+Shift+K',
    handler: editorDeleteLine,
  },
  {
    id: 'editor.lockGroup',
    title: 'Lock Editor Group',
    category: 'View',
    handler: toggleLockActiveGroup,
  },
  {
    id: 'editor.maximizeGroup',
    title: 'Maximize Editor Group',
    category: 'View',
    defaultKeybinding: 'Ctrl+K Ctrl+M',
    handler: toggleMaximizeActiveGroup,
  },
  {
    id: 'workbench.showOpenedEditors',
    title: 'Show Opened Editors',
    category: 'View',
    handler: showOpenedEditors,
  },
  {
    id: 'selection.selectAll',
    title: 'Select All',
    category: 'Selection',
    handler: editorSelectAll,
  },
  {
    id: 'selection.selectNextMatch',
    title: 'Select Next Match',
    category: 'Selection',
    defaultKeybinding: 'Ctrl+D',
    handler: editorSelectNextMatch,
  },
  {
    id: 'selection.selectAllMatches',
    title: 'Select All Matches',
    category: 'Selection',
    defaultKeybinding: 'Ctrl+Shift+L',
    handler: editorSelectAllMatches,
  },
  {
    id: 'selection.addCursorAbove',
    title: 'Add Cursor Above',
    category: 'Selection',
    handler: editorAddCursorAbove,
  },
  {
    id: 'selection.addCursorBelow',
    title: 'Add Cursor Below',
    category: 'Selection',
    handler: editorAddCursorBelow,
  },
  {
    id: 'view.toggleSidebar',
    title: 'Toggle Sidebar',
    category: 'View',
    defaultKeybinding: 'Ctrl+B',
    handler: () => togglePanel('fileTree'),
  },
  {
    id: 'panel.explorer',
    title: 'Toggle Explorer',
    category: 'View',
    defaultKeybinding: 'Ctrl+B',
    handler: () => togglePanel('fileTree'),
  },
  {
    id: 'view.toggleBottomPanel',
    title: 'Toggle Bottom Panel',
    category: 'View',
    defaultKeybinding: 'Ctrl+J',
    handler: toggleTerminalPanel,
  },
  {
    id: 'panel.terminal',
    title: 'Toggle Terminal',
    category: 'View',
    defaultKeybinding: 'Ctrl+`',
    handler: toggleTerminalPanel,
  },
  {
    id: 'terminal.new',
    title: 'New Terminal',
    category: 'Terminal',
    defaultKeybinding: 'Ctrl+Shift+`',
    handler: newTerminal,
  },
  {
    id: 'terminal.split',
    title: 'Split Terminal',
    category: 'Terminal',
    handler: newTerminal,
  },
  {
    id: 'terminal.clear',
    title: 'Clear Terminal',
    category: 'Terminal',
    handler: clearActiveTerminal,
  },
  {
    id: 'terminal.kill',
    title: 'Kill Terminal',
    category: 'Terminal',
    handler: killActiveTerminal,
  },
  {
    id: 'terminal.focus',
    title: 'Focus Terminal',
    category: 'Terminal',
    handler: focusTerminal,
  },
  {
    id: 'view.showExplorer',
    title: 'Toggle Explorer',
    category: 'View',
    handler: () => showSidebarView('explorer'),
  },
  {
    id: 'view.openWebPreview',
    title: 'Open Web Preview',
    category: 'View',
    defaultKeybinding: 'Ctrl+Shift+W',
    handler: () => void openWebPreview(),
  },
  {
    id: 'workbench.openWebPreview',
    title: 'Open Web Preview',
    category: 'View',
    defaultKeybinding: 'Ctrl+Shift+W',
    handler: () => void openWebPreview(),
  },
  {
    id: 'go.definition',
    title: 'Go to Definition',
    category: 'Go',
    defaultKeybinding: 'F12',
    handler: editorGoToDefinition,
  },
  {
    id: 'go.line',
    title: 'Go to Line',
    category: 'Go',
    handler: editorGoToLine,
  },
  {
    id: 'go.back',
    title: 'Back',
    category: 'Go',
    handler: () => noop('Back'),
  },
  {
    id: 'go.forward',
    title: 'Forward',
    category: 'Go',
    handler: () => noop('Forward'),
  },
  {
    id: 'editor.renameSymbol',
    title: 'Rename Symbol',
    category: 'Go',
    defaultKeybinding: 'F2',
    handler: editorRenameSymbol,
  },
  {
    id: 'run.task',
    title: 'Run Task',
    category: 'Run',
    handler: () => noop('Run Task'),
  },
  {
    id: 'settings.open',
    title: 'Open Settings',
    category: 'Preferences',
    defaultKeybinding: 'Ctrl+,',
    handler: openSettings,
  },
  {
    id: 'ai.openChat',
    title: 'Open AI Chat',
    category: 'AI',
    defaultKeybinding: 'Ctrl+L',
    handler: ensureAiOpen,
  },
  {
    id: 'ai.inlinePrompt',
    title: 'Inline AI Prompt',
    category: 'AI',
    defaultKeybinding: 'Ctrl+I',
    handler: ensureAiOpen,
  },
  {
    id: 'ai.explainSelection',
    title: 'AI: Explain Selection',
    category: 'AI',
    defaultKeybinding: 'Ctrl+Alt+E',
    handler: explainSelection,
  },
  {
    id: 'ai.fixSelection',
    title: 'AI: Fix Selection',
    category: 'AI',
    defaultKeybinding: 'Ctrl+Alt+F',
    handler: fixSelection,
  },
  {
    id: 'ai.generateTests',
    title: 'AI: Generate Tests',
    category: 'AI',
    defaultKeybinding: 'Ctrl+Alt+T',
    handler: generateTestsForSelection,
  },
  {
    id: 'git.checkoutBranch',
    title: 'Git: Checkout Branch',
    category: 'Git',
    handler: () => void checkoutBranch(),
  },
  {
    id: 'git.createBranch',
    title: 'Git: Create Branch',
    category: 'Git',
    handler: () => void createBranch(),
  },
  {
    id: 'git.deleteBranch',
    title: 'Git: Delete Branch',
    category: 'Git',
    handler: () => void deleteBranch(),
  },
  {
    id: 'help.about',
    title: 'About',
    category: 'Help',
    handler: () =>
      alert(
        'GwenLand IDE\nVersion 0.1.0\n\nA lightweight, local-first code editor built with Tauri + Rust.',
      ),
  },
  {
    id: 'help.keyboardShortcuts',
    title: 'Keyboard Shortcuts',
    category: 'Help',
    handler: openPalette,
  },
]

export function registerCommands(): void {
  byId.clear()
  for (const command of COMMANDS) register(command)
  commands.set([...byId.values()])
}

export function commandById(id: string): Command | undefined {
  return byId.get(id)
}

export function allCommands(): Command[] {
  return [...byId.values()]
}

export async function runCommand(id: string): Promise<boolean> {
  const command = byId.get(id)
  if (!command) return false
  await command.handler()
  return true
}

export function filterCommands(query: string): Command[] {
  const q = query.trim().toLowerCase()
  const all = get(commands)
  if (!q) return all
  return all.filter(
    (command) =>
      command.title.toLowerCase().includes(q) ||
      command.id.toLowerCase().includes(q) ||
      command.category.toLowerCase().includes(q),
  )
}

export function keybindingsFor(command: Command): string[] {
  return (command.defaultKeybinding ?? '')
    .split(/\s+\/\s+/)
    .map((keybinding) => keybinding.trim())
    .filter(Boolean)
}

export function shortcutFor(id: string): string | undefined {
  const command = byId.get(id)
  return command ? keybindingsFor(command).at(0) : undefined
}

export function commandCategory(id: string): string {
  return byId.get(id)?.category ?? ''
}

export const MENUS: { name: string; items: MenuItem[] }[] = [
  {
    name: 'File',
    items: [
      { label: 'New File', commandId: 'file.new' },
      { label: 'Open Folder', commandId: 'file.openFolder' },
      { type: 'divider' },
      { label: 'Open Recent', children: 'recent' },
      { type: 'divider' },
      { label: 'Save', commandId: 'file.save' },
      { label: 'Save As', commandId: 'file.saveAs' },
      { type: 'divider' },
      { label: 'Create Local History Snapshot', commandId: 'fileHistory.createSnapshot' },
      { label: 'Show Local History', commandId: 'fileHistory.show' },
      { label: 'Clear Local History', commandId: 'fileHistory.clearHistory' },
      { type: 'divider' },
      { label: 'Close Tab', commandId: 'tab.close' },
      { label: 'Close All', commandId: 'tab.closeAll' },
    ],
  },
  {
    name: 'Edit',
    items: [
      { label: 'Undo', commandId: 'edit.undo', shortcut: 'Ctrl+Z' },
      { label: 'Redo', commandId: 'edit.redo', shortcut: 'Ctrl+Y' },
      { type: 'divider' },
      { label: 'Cut', commandId: 'edit.cut', shortcut: 'Ctrl+X' },
      { label: 'Copy', commandId: 'edit.copy', shortcut: 'Ctrl+C' },
      { label: 'Paste', commandId: 'edit.paste', shortcut: 'Ctrl+V' },
      { type: 'divider' },
      { label: 'Find', commandId: 'search.inFile' },
      { label: 'Replace', commandId: 'search.replaceInFile' },
      { label: 'Toggle Comment', commandId: 'editor.toggleComment' },
    ],
  },
  {
    name: 'Selection',
    items: [
      { label: 'Select All', commandId: 'selection.selectAll', shortcut: 'Ctrl+A' },
      { label: 'Select Next Match', commandId: 'selection.selectNextMatch' },
      { label: 'Select All Matches', commandId: 'selection.selectAllMatches' },
      { label: 'Add Cursor Above', commandId: 'selection.addCursorAbove', shortcut: 'Ctrl+Alt+Up' },
      { label: 'Add Cursor Below', commandId: 'selection.addCursorBelow', shortcut: 'Ctrl+Alt+Down' },
    ],
  },
  {
    name: 'View',
    items: [
      { label: 'Toggle Sidebar', commandId: 'view.toggleSidebar' },
      { label: 'Toggle Terminal', commandId: 'panel.terminal' },
      { label: 'Toggle Explorer', commandId: 'view.showExplorer' },
      { type: 'divider' },
      { label: 'Close Saved in Group', commandId: 'editor.closeSaved' },
      { label: 'Lock Editor Group', commandId: 'editor.lockGroup' },
      { label: 'Maximize Editor Group', commandId: 'editor.maximizeGroup' },
      { label: 'Show Opened Editors', commandId: 'workbench.showOpenedEditors' },
      { type: 'divider' },
      { label: 'Command Palette', commandId: 'workbench.action.showCommands' },
      { label: 'Open Web Preview', commandId: 'workbench.openWebPreview' },
    ],
  },
  {
    name: 'Go',
    items: [
      { label: 'Go to Definition', commandId: 'go.definition' },
      { label: 'Go to Line', commandId: 'go.line' },
      { label: 'Back', commandId: 'go.back' },
      { label: 'Forward', commandId: 'go.forward' },
    ],
  },
  {
    name: 'Run',
    items: [{ label: 'Run Task', commandId: 'run.task' }],
  },
  {
    name: 'Terminal',
    items: [
      { label: 'New Terminal', commandId: 'terminal.new' },
      { label: 'Split Terminal', commandId: 'terminal.split' },
      { label: 'Clear Terminal', commandId: 'terminal.clear' },
      { label: 'Kill Terminal', commandId: 'terminal.kill' },
    ],
  },
  {
    name: 'Help',
    items: [
      { label: 'About', commandId: 'help.about' },
      { label: 'Keyboard Shortcuts', commandId: 'help.keyboardShortcuts' },
    ],
  },
]
