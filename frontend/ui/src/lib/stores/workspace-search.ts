import { derived, get, writable } from 'svelte/store'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  onWorkspaceSearchDone,
  onWorkspaceSearchResult,
  searchCancel,
  searchWorkspace,
  type WorkspaceSearchDoneEvent,
  type WorkspaceSearchResult,
} from '../tauri/commands'
import { showSidebarView } from './sidebar'

export interface WorkspaceSearchState {
  query: string
  activeSearchId: string | null
  searching: boolean
  results: WorkspaceSearchResult[]
  scannedFiles: number
  truncated: boolean
  error: string | null
}

export interface WorkspaceSearchGroup {
  path: string
  relativePath: string
  results: WorkspaceSearchResult[]
}

const initial: WorkspaceSearchState = {
  query: '',
  activeSearchId: null,
  searching: false,
  results: [],
  scannedFiles: 0,
  truncated: false,
  error: null,
}

export const workspaceSearch = writable<WorkspaceSearchState>(initial)

export function groupWorkspaceSearchResults(results: WorkspaceSearchResult[]): WorkspaceSearchGroup[] {
  const byPath = new Map<string, WorkspaceSearchGroup>()
  for (const result of results) {
    let group = byPath.get(result.path)
    if (!group) {
      group = {
        path: result.path,
        relativePath: result.relative_path,
        results: [],
      }
      byPath.set(result.path, group)
    }
    group.results.push(result)
  }
  return [...byPath.values()]
}

export const workspaceSearchGroups = derived(workspaceSearch, ($search) =>
  groupWorkspaceSearchResults($search.results)
)

let listenersReady: Promise<void> | null = null
let unlistenResult: UnlistenFn | null = null
let unlistenDone: UnlistenFn | null = null

function genSearchId(): string {
  return crypto.randomUUID
    ? `search-${crypto.randomUUID()}`
    : `search-${Date.now()}-${Math.random().toString(16).slice(2)}`
}

async function ensureSearchListeners(): Promise<void> {
  if (listenersReady) return listenersReady
  listenersReady = Promise.all([
    onWorkspaceSearchResult((event) => {
      workspaceSearch.update((state) => {
        if (state.activeSearchId !== event.search_id) return state
        return { ...state, results: [...state.results, event.result] }
      })
    }),
    onWorkspaceSearchDone((event: WorkspaceSearchDoneEvent) => {
      workspaceSearch.update((state) => {
        if (state.activeSearchId !== event.search_id) return state
        return {
          ...state,
          activeSearchId: null,
          searching: false,
          scannedFiles: event.summary?.scanned_files ?? state.scannedFiles,
          truncated: event.summary?.truncated ?? false,
          error: event.error,
        }
      })
    }),
  ]).then(([result, done]) => {
    unlistenResult = result
    unlistenDone = done
  })
  return listenersReady
}

export function openWorkspaceSearch(): void {
  showSidebarView('search')
}

export function setWorkspaceSearchQuery(query: string): void {
  workspaceSearch.update((state) => ({ ...state, query }))
}

export function clearWorkspaceSearchResults(): void {
  workspaceSearch.update((state) => ({
    ...state,
    activeSearchId: null,
    searching: false,
    results: [],
    scannedFiles: 0,
    truncated: false,
    error: null,
  }))
}

export async function cancelWorkspaceSearch(): Promise<void> {
  const id = get(workspaceSearch).activeSearchId
  if (id) await searchCancel(id).catch(() => {})
  workspaceSearch.update((state) => ({
    ...state,
    activeSearchId: null,
    searching: false,
  }))
}

export async function runWorkspaceSearch(root: string, query: string): Promise<void> {
  const trimmed = query.trim()
  await ensureSearchListeners()
  await cancelWorkspaceSearch()

  if (!root || !trimmed) {
    clearWorkspaceSearchResults()
    return
  }

  const searchId = genSearchId()
  workspaceSearch.set({
    query,
    activeSearchId: searchId,
    searching: true,
    results: [],
    scannedFiles: 0,
    truncated: false,
    error: null,
  })

  try {
    await searchWorkspace(root, trimmed, searchId)
  } catch (e) {
    workspaceSearch.update((state) =>
      state.activeSearchId === searchId
        ? { ...state, activeSearchId: null, searching: false, error: String(e) }
        : state
    )
  }
}

export function disposeWorkspaceSearchListeners(): void {
  unlistenResult?.()
  unlistenResult = null
  unlistenDone?.()
  unlistenDone = null
  listenersReady = null
}
