import { writable, get } from 'svelte/store'
import { activeDoc, replaceActiveDocument } from '../editor/active-editor'
import {
  historyClear,
  historyList,
  historyReadEntry,
  readFile,
  writeFile,
  type HistoryEntry,
} from '../tauri/commands'
import { isEditorTab, setTabContent, tabs } from './tabs'
import { workspace } from './workspace'
import { createHistorySnapshot } from './history-snapshots'

export interface LocalHistoryState {
  open: boolean
  filePath: string | null
  currentContent: string
  entries: HistoryEntry[]
  selectedTimestamp: string | null
  selectedContent: string
  loading: boolean
  error: string | null
}

const initial: LocalHistoryState = {
  open: false,
  filePath: null,
  currentContent: '',
  entries: [],
  selectedTimestamp: null,
  selectedContent: '',
  loading: false,
  error: null,
}

export const localHistory = writable<LocalHistoryState>(initial)

function activeEditorTab() {
  const state = get(tabs)
  const tab = state.tabs.find((candidate) => candidate.id === state.activeId)
  return tab && isEditorTab(tab) ? tab : null
}

async function loadEntries(filePath: string): Promise<void> {
  const root = get(workspace).folderPath
  if (!root) return
  localHistory.update((s) => ({ ...s, loading: true, error: null }))
  try {
    const entries = await historyList(root, filePath)
    localHistory.update((s) => ({
      ...s,
      entries,
      selectedTimestamp: entries[0]?.timestamp ?? null,
      selectedContent: '',
      loading: false,
    }))
    if (entries[0]) await selectHistoryEntry(entries[0].timestamp)
  } catch (e) {
    localHistory.update((s) => ({ ...s, loading: false, error: String(e) }))
  }
}

export async function openLocalHistory(filePath?: string): Promise<void> {
  const tab = activeEditorTab()
  const target = filePath ?? tab?.path
  if (!target) return
  let currentContent =
    tab?.id === get(tabs).activeId
      ? activeDoc() ?? tab.state.doc.toString()
      : tab?.state.doc.toString() ?? ''
  if (!currentContent && !tab) {
    currentContent = await readFile(target).catch(() => '')
  }
  localHistory.set({
    ...initial,
    open: true,
    filePath: target,
    currentContent,
  })
  await loadEntries(target)
}

export function closeLocalHistory(): void {
  localHistory.update((s) => ({ ...s, open: false }))
}

export async function refreshLocalHistory(): Promise<void> {
  const filePath = get(localHistory).filePath
  if (filePath) await loadEntries(filePath)
}

export async function selectHistoryEntry(timestamp: string): Promise<void> {
  const state = get(localHistory)
  const root = get(workspace).folderPath
  if (!root || !state.filePath) return
  localHistory.update((s) => ({ ...s, selectedTimestamp: timestamp, loading: true, error: null }))
  try {
    const selectedContent = await historyReadEntry(root, state.filePath, timestamp)
    localHistory.update((s) => ({ ...s, selectedContent, loading: false }))
  } catch (e) {
    localHistory.update((s) => ({ ...s, loading: false, error: String(e) }))
  }
}

export async function createManualHistorySnapshot(): Promise<void> {
  const tab = activeEditorTab()
  if (!tab?.path) return
  const content = activeDoc() ?? tab.state.doc.toString()
  await createHistorySnapshot(tab.path, content, 'manual')
  await openLocalHistory(tab.path)
}

export async function restoreSelectedHistory(): Promise<void> {
  const state = get(localHistory)
  const tab = activeEditorTab()
  if (!state.filePath || !state.selectedTimestamp) return
  await writeFile(state.filePath, state.selectedContent)
  if (tab && tab.path === state.filePath) {
    replaceActiveDocument(state.selectedContent)
    setTabContent(tab.id, state.selectedContent)
  }
  await createHistorySnapshot(state.filePath, state.selectedContent, 'manual')
  await openLocalHistory(state.filePath)
}

export async function clearLocalHistory(): Promise<void> {
  const state = get(localHistory)
  const root = get(workspace).folderPath
  if (!root || !state.filePath) return
  if (!confirm('Clear local history for this file?')) return
  await historyClear(root, state.filePath)
  await openLocalHistory(state.filePath)
}
