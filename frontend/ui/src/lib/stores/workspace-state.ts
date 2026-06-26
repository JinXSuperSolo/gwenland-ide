import { get } from 'svelte/store'
import { aiChat, type ReasoningLevel } from './ai-chat'
import { panels } from './panels'
import { settings, setSettings, THEME_PRESETS } from './settings'
import { terminalSessions, createSession } from './terminal-sessions'
import {
  activateTab,
  editorGroupsSnapshot,
  isDiffTab,
  isEditorTab,
  isPreviewTab,
  openDiff,
  openFile,
  openPreview,
  resetEditorGroups,
  setActiveGroup,
  tabs,
  type Tab,
} from './tabs'
import { workspace } from './workspace'
import {
  loadLayoutState,
  loadWorkspaceState,
  pathExists,
  saveLayoutState as saveLayoutStateCommand,
  saveWorkspaceState as saveWorkspaceStateCommand,
  type PersistedConversationState,
  type PersistedLayoutState,
  type PersistedWorkspaceState,
  type PersistedWorkspaceTab,
} from '../tauri/commands'

const SAVE_DELAY_MS = 500

let initialized = false
let restoring = false
let restoredRoot: string | null = null
let workspaceTimer: ReturnType<typeof setTimeout> | null = null
let layoutTimer: ReturnType<typeof setTimeout> | null = null

function activeWorkspaceRoot(): string | null {
  return get(workspace).folderPath
}

function tabRestoreKey(tab: Tab): string {
  if (isEditorTab(tab)) return tab.path
  if (isPreviewTab(tab)) return tab.source.kind === 'static-file' ? tab.source.path : tab.source.url
  if (isDiffTab(tab)) return tab.path
  return ''
}

function persistedTab(tab: Tab): PersistedWorkspaceTab | null {
  const path = tabRestoreKey(tab)
  if (!path) return null
  return {
    path,
    type: tab.kind,
    isDirty: isEditorTab(tab) ? tab.dirty : false,
    isPreview: !!tab.preview,
  }
}

function conversationState(): PersistedConversationState {
  const s = get(aiChat)
  return {
    isOpen: s.isOpen,
    activeConversationId: s.activeConversationId,
    activeProvider: s.activeProvider,
    activeModel: s.activeModel,
    reasoningLevel: s.reasoningLevel,
    unsentInput: s.unsentInput,
  }
}

function currentWorkspaceState(root: string): PersistedWorkspaceState {
  const tabState = get(tabs)
  const active = tabState.tabs.find((tab) => tab.id === tabState.activeId)
  return {
    workspaceRoot: root,
    openTabs: tabState.tabs.map(persistedTab).filter((tab): tab is PersistedWorkspaceTab => !!tab),
    activeTabPath: active ? tabRestoreKey(active) : '',
    conversationState: conversationState(),
  }
}

function currentLayoutState(): PersistedLayoutState {
  const panelState = get(panels)
  const terminals = get(terminalSessions)
  const tabState = get(tabs)
  return {
    sidebarOpen: !panelState.fileTree.collapsed,
    sidebarWidth: panelState.fileTree.size,
    bottomPanelOpen: !panelState.terminal.collapsed,
    bottomPanelHeight: panelState.terminal.size,
    terminalOpen: !panelState.terminal.collapsed && terminals.sessions.length > 0,
    theme: get(settings).preset,
    editorGroupOrientation: tabState.orientation,
    activeEditorGroupId: tabState.activeGroupId,
    editorGroups: editorGroupsSnapshot(),
  }
}

function scheduleWorkspaceSave(): void {
  if (restoring) return
  const root = activeWorkspaceRoot()
  if (!root) return
  if (workspaceTimer) clearTimeout(workspaceTimer)
  workspaceTimer = setTimeout(() => {
    workspaceTimer = null
    const latestRoot = activeWorkspaceRoot()
    if (!latestRoot || restoring) return
    void saveWorkspaceStateCommand(latestRoot, currentWorkspaceState(latestRoot)).catch(() => {})
  }, SAVE_DELAY_MS)
}

function scheduleLayoutSave(): void {
  if (restoring) return
  const root = activeWorkspaceRoot()
  if (!root) return
  if (layoutTimer) clearTimeout(layoutTimer)
  layoutTimer = setTimeout(() => {
    layoutTimer = null
    const latestRoot = activeWorkspaceRoot()
    if (!latestRoot || restoring) return
    void saveLayoutStateCommand(latestRoot, currentLayoutState()).catch(() => {})
  }, SAVE_DELAY_MS)
}

function validReasoning(value: unknown): value is ReasoningLevel {
  return value === 'low' || value === 'medium' || value === 'high' || value === 'extra_high'
}

function restoreConversation(state: PersistedConversationState | null | undefined): void {
  if (!state) return
  aiChat.update((s) => ({
    ...s,
    isOpen: typeof state.isOpen === 'boolean' ? state.isOpen : s.isOpen,
    activeConversationId:
      state.activeConversationId === undefined ? s.activeConversationId : state.activeConversationId,
    activeProvider: state.activeProvider || s.activeProvider,
    activeModel: state.activeModel || s.activeModel,
    reasoningLevel: validReasoning(state.reasoningLevel) ? state.reasoningLevel : s.reasoningLevel,
    unsentInput: typeof state.unsentInput === 'string' ? state.unsentInput : s.unsentInput,
  }))
}

function restoreLayout(state: PersistedLayoutState | null): void {
  if (!state) return
  panels.update((s) => ({
    ...s,
    fileTree: {
      ...s.fileTree,
      size: Number.isFinite(state.sidebarWidth) ? state.sidebarWidth : s.fileTree.size,
      collapsed: !state.sidebarOpen,
    },
    terminal: {
      ...s.terminal,
      size: Number.isFinite(state.bottomPanelHeight)
        ? state.bottomPanelHeight
        : s.terminal.size,
      collapsed: !(state.bottomPanelOpen || state.terminalOpen),
    },
  }))

  if (state.theme && THEME_PRESETS[state.theme]) {
    setSettings({ preset: state.theme })
  }

  if (state.terminalOpen && get(terminalSessions).sessions.length === 0) {
    createSession()
  }
}

function isDevServerUrl(path: string): boolean {
  return /^https?:\/\//i.test(path)
}

function portFromUrl(url: string): number | null {
  try {
    const parsed = new URL(url)
    const port = Number(parsed.port)
    return Number.isInteger(port) && port > 0 ? port : null
  } catch {
    return null
  }
}

async function restoreTab(
  tab: PersistedWorkspaceTab,
  root: string,
  groupId?: string,
): Promise<void> {
  if (!tab.path) return
  if (tab.type === 'preview' && isDevServerUrl(tab.path)) {
    const port = portFromUrl(tab.path)
    if (port) openPreview({ kind: 'dev-server', url: tab.path, port }, { groupId, preview: tab.isPreview, ignoreLock: true })
    return
  }

  const exists = await pathExists(tab.path).catch(() => false)
  if (!exists) return

  if (tab.type === 'preview') {
    openPreview({ kind: 'static-file', path: tab.path }, { groupId, preview: tab.isPreview, ignoreLock: true })
  } else if (tab.type === 'diff') {
    openDiff(root, tab.path, false, groupId, true)
  } else {
    await openFile(tab.path, { groupId, preview: tab.isPreview, ignoreLock: true })
  }
}

async function restoreWorkspace(
  root: string,
  state: PersistedWorkspaceState | null,
  layoutState: PersistedLayoutState | null,
): Promise<void> {
  if (!state || state.workspaceRoot !== root) return

  const groupLayouts = layoutState?.editorGroups?.length ? layoutState.editorGroups : null
  if (groupLayouts) {
    resetEditorGroups(
      groupLayouts.map((group) => ({
        id: group.id,
        isLocked: false,
        isMaximized: false,
        size: group.size ?? 1,
      })),
      layoutState?.editorGroupOrientation ?? 'horizontal',
      layoutState?.activeEditorGroupId,
    )

    for (const group of groupLayouts) {
      for (const tab of group.tabs ?? []) {
        await restoreTab(tab, root, group.id)
      }
      if (group.activeTabPath) {
        const restoredGroup = get(tabs).groups.find((candidate) => candidate.id === group.id)
        const active = restoredGroup?.tabs.find((tab) => tabRestoreKey(tab) === group.activeTabPath)
        if (active) activateTab(active.id, group.id)
      }
    }

    if (layoutState?.activeEditorGroupId) {
      setActiveGroup(layoutState.activeEditorGroupId)
    }
    restoreConversation(state.conversationState)
    return
  }

  for (const tab of state.openTabs ?? []) {
    await restoreTab(tab, root)
  }

  if (state.activeTabPath) {
    const match = get(tabs).tabs.find((tab) => tabRestoreKey(tab) === state.activeTabPath)
    if (match) activateTab(match.id)
  }

  restoreConversation(state.conversationState)
}

async function restoreForRoot(root: string): Promise<void> {
  if (restoredRoot === root) return
  restoredRoot = root
  restoring = true
  try {
    const [workspaceState, layoutState] = await Promise.all([
      loadWorkspaceState(root).catch(() => null),
      loadLayoutState(root).catch(() => null),
    ])
    restoreLayout(layoutState)
    await restoreWorkspace(root, workspaceState, layoutState)
  } finally {
    restoring = false
  }
}

export function initWorkspaceStatePersistence(): void {
  if (initialized) return
  initialized = true

  workspace.subscribe((s) => {
    if (s.folderPath) void restoreForRoot(s.folderPath)
  })
  tabs.subscribe(() => scheduleWorkspaceSave())
  aiChat.subscribe(() => scheduleWorkspaceSave())
  panels.subscribe(() => scheduleLayoutSave())
  settings.subscribe(() => scheduleLayoutSave())
  terminalSessions.subscribe(() => scheduleLayoutSave())
}
