import { get, writable } from 'svelte/store'
import { getCommitDetails, getGitGraph } from '../tauri/commands'
import type { CommitDetails, CommitGraphPayload } from '../types/git'

export interface GitGraphState {
  workspacePath: string | null
  payload: CommitGraphPayload | null
  loading: boolean
  error: string | null
}

export interface CommitDetailsState {
  value: CommitDetails | null
  loading: boolean
  error: string | null
}

const initial: GitGraphState = {
  workspacePath: null,
  payload: null,
  loading: false,
  error: null,
}

export const gitGraph = writable<GitGraphState>(initial)
export const commitDetailsCache = writable<Record<string, CommitDetailsState>>({})

let requestSerial = 0
const detailRequests = new Map<string, Promise<void>>()

export function commitDetailsKey(workspacePath: string, hash: string): string {
  return `${workspacePath}\0${hash}`
}

export async function loadCommitDetails(workspacePath: string, hash: string): Promise<void> {
  const key = commitDetailsKey(workspacePath, hash)
  const existing = get(commitDetailsCache)[key]
  if (existing?.value) return
  const inFlight = detailRequests.get(key)
  if (inFlight) return inFlight

  const request = (async () => {
    commitDetailsCache.update((state) => ({
      ...state,
      [key]: { value: state[key]?.value ?? null, loading: true, error: null },
    }))
    try {
      const value = await getCommitDetails(workspacePath, hash)
      commitDetailsCache.update((state) => ({
        ...state,
        [key]: { value, loading: false, error: null },
      }))
    } catch (e) {
      commitDetailsCache.update((state) => ({
        ...state,
        [key]: { value: state[key]?.value ?? null, loading: false, error: String(e) },
      }))
    } finally {
      detailRequests.delete(key)
    }
  })()

  detailRequests.set(key, request)
  return request
}

export async function loadGitGraph(
  workspacePath: string,
  maxCommits = 300,
  force = false
): Promise<void> {
  const serial = ++requestSerial
  let shouldLoad = force

  gitGraph.update((state) => {
    shouldLoad =
      shouldLoad ||
      state.workspacePath !== workspacePath ||
      (!state.payload && !state.loading)

    if (!shouldLoad) return state
    return {
      workspacePath,
      payload: state.workspacePath === workspacePath ? state.payload : null,
      loading: true,
      error: null,
    }
  })

  if (!shouldLoad) return

  try {
    const payload = await getGitGraph(workspacePath, maxCommits)
    if (serial !== requestSerial) return
    gitGraph.set({
      workspacePath,
      payload,
      loading: false,
      error: null,
    })
  } catch (e) {
    if (serial !== requestSerial) return
    gitGraph.set({
      workspacePath,
      payload: null,
      loading: false,
      error: String(e),
    })
  }
}

export function refreshGitGraph(workspacePath: string, maxCommits = 300): Promise<void> {
  return loadGitGraph(workspacePath, maxCommits, true)
}

export function clearGitGraph(): void {
  requestSerial += 1
  gitGraph.set(initial)
}
