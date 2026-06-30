import { writable } from 'svelte/store'
import { collapsePanel, expandPanel } from './panels'

/**
 * Which view the left sidebar shows (Wave 2 — GWEN-328 adds Source Control next
 * to the existing Explorer). The file-tree panel slot renders one of these.
 */
export type SidebarView = 'explorer' | 'search' | 'git'
export type SidebarTab = 'files' | 'agent'

export const sidebarView = writable<SidebarView>('explorer')
export const sidebarTab = writable<SidebarTab>('files')

const SIDEBAR_TAB_STORAGE_KEY = 'gwenland.sidebarTab'
let persistenceReady = false

function parseSidebarTab(value: string | null): SidebarTab | null {
  return value === 'files' || value === 'agent' ? value : null
}

export function initSidebarTabPersistence(): void {
  if (persistenceReady || typeof window === 'undefined') return
  persistenceReady = true

  const stored = parseSidebarTab(window.localStorage.getItem(SIDEBAR_TAB_STORAGE_KEY))
  if (stored) sidebarTab.set(stored)

  sidebarTab.subscribe((tab) => {
    window.localStorage.setItem(SIDEBAR_TAB_STORAGE_KEY, tab)
  })

  window.addEventListener('storage', (event) => {
    if (event.key !== SIDEBAR_TAB_STORAGE_KEY) return
    const next = parseSidebarTab(event.newValue)
    if (next) sidebarTab.set(next)
  })
}

/** Switch the sidebar view and make sure the file-tree panel is expanded. */
export function showSidebarView(view: SidebarView): void {
  sidebarView.set(view)
  if (view === 'explorer') sidebarTab.set('files')
  expandPanel('fileTree')
}

export function showFilesTab(): void {
  sidebarView.set('explorer')
  sidebarTab.set('files')
  expandPanel('fileTree')
}

export function showAgentTab(): void {
  sidebarView.set('explorer')
  sidebarTab.set('agent')
  expandPanel('fileTree')
}

export function collapseSidebar(): void {
  collapsePanel('fileTree')
}
