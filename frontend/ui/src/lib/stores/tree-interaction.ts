import { writable, type Unsubscriber } from 'svelte/store'
import { isEditorTab, isPreviewTab, tabs, type TabsState } from './tabs'

export interface TreeInteractionState {
  selectedId: string | null
  focusedId: string | null
  activeEditorPath: string | null
}

const initial: TreeInteractionState = {
  selectedId: null,
  focusedId: null,
  activeEditorPath: null,
}

export const treeInteraction = writable<TreeInteractionState>(initial)

let tabsUnsub: Unsubscriber | null = null

export function selectRow(id: string | null): void {
  treeInteraction.update((state) => ({ ...state, selectedId: id }))
}

export function focusRow(id: string | null): void {
  treeInteraction.update((state) => ({ ...state, focusedId: id }))
}

export function setActiveEditor(path: string | null): void {
  treeInteraction.update((state) => ({ ...state, activeEditorPath: path }))
}

export function activePathFromTabs(state: TabsState): string | null {
  const active = state.tabs.find((tab) => tab.id === state.activeId)
  if (!active) return null
  if (isEditorTab(active)) return active.path || null
  if (isPreviewTab(active) && active.source.kind === 'static-file') return active.source.path
  return null
}

export function initTreeInteraction(): void {
  if (tabsUnsub) return
  tabsUnsub = tabs.subscribe((state) => {
    setActiveEditor(activePathFromTabs(state))
  })
}

export function disposeTreeInteraction(): void {
  if (tabsUnsub) {
    tabsUnsub()
    tabsUnsub = null
  }
}

export function resetTreeInteraction(): void {
  treeInteraction.set(initial)
}
