import { writable, get } from 'svelte/store'
import { workspace } from './workspace'
import { subscribeFocus, isAppActive } from './app-focus'
import { gitIsRepo, gitStatus, type GitFileStatus } from '../tauri/commands'

/**
 * Git status state (Wave 2 — GWEN-327/329). Polls the open workspace every 4s
 * (and on demand after any git action) so the status bar, file-tree colors, and
 * Git panel stay in sync. When the open folder isn't a git repo, `isRepo` is
 * false and all git UI hides.
 *
 * The store is Tauri-light: it only calls the read-only `git_status` wrapper.
 * Mutations (stage/commit/…) live in the Git panel and call `refreshGit()` after.
 */
export interface GitState {
  /** Whether the open folder is a git work tree. */
  isRepo: boolean
  /** Current branch (or detached-HEAD label). */
  branch: string
  /** Count of changed/untracked entries. */
  dirtyCount: number
  /** Per-file status, keyed for quick lookup by the file tree. */
  files: GitFileStatus[]
}

const initial: GitState = { isRepo: false, branch: '', dirtyCount: 0, files: [] }

export const git = writable<GitState>(initial)

/** Poll cadence in ms. 10s keeps idle cost low; the 4s version was wasteful. */
const POLL_MS = 10000

let pollTimer: ReturnType<typeof setInterval> | null = null
let lastRoot: string | null = null

/** Re-read git status for the open workspace. No-op when no folder is open. */
export async function refreshGit(): Promise<void> {
  const root = get(workspace).folderPath
  if (!root) {
    git.set(initial)
    return
  }
  try {
    const isRepo = await gitIsRepo(root)
    if (!isRepo) {
      git.set({ ...initial, isRepo: false })
      return
    }
    const status = await gitStatus(root)
    git.set({
      isRepo: true,
      branch: status.branch,
      dirtyCount: status.dirty_count,
      files: status.files,
    })
  } catch {
    // Treat any failure (git missing, transient) as "not a repo" so the UI hides
    // rather than showing a broken indicator.
    git.set(initial)
  }
}

/** Whether polling should currently run: a folder is open AND the app is active. */
function shouldPoll(): boolean {
  return get(workspace).folderPath !== null && isAppActive()
}

/** (Re)start the interval if it should run and isn't already. Idempotent. */
function startPolling(): void {
  if (pollTimer || !shouldPoll()) return
  pollTimer = setInterval(() => void refreshGit(), POLL_MS)
}

/** Stop the interval (no-op if already stopped). */
function stopPolling(): void {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

let inited = false

/**
 * Wire git polling. It runs ONLY while a workspace is open and the window is
 * active; it pauses on blur/hidden and resumes (with an immediate refresh) on
 * focus. Idempotent.
 */
export function initGit(): void {
  if (inited) return
  inited = true

  // Open/close a folder: refresh immediately and (re)evaluate whether to poll.
  // This fires synchronously on subscribe, so a folder already open at startup
  // is handled here too (no separate kick-off needed).
  workspace.subscribe((s) => {
    if (s.folderPath !== lastRoot) {
      lastRoot = s.folderPath
      if (s.folderPath) {
        void refreshGit()
        startPolling()
      } else {
        stopPolling()
        git.set(initial)
      }
    }
  })

  // Background throttle: pause the interval on blur/hidden, resume + refresh
  // immediately on focus/visible (so a return shows fresh status at once).
  subscribeFocus((active) => {
    if (active) {
      if (get(workspace).folderPath) void refreshGit()
      startPolling()
    } else {
      stopPolling()
    }
  })
}

/** A repo-relative status lookup for the file tree (GWEN-329). Returns the badge
 *  letter for an absolute file path, or null when clean/not-a-repo. */
export function statusForPath(absPath: string, root: string): GitFileStatus | null {
  const state = get(git)
  if (!state.isRepo) return null
  // Normalize the absolute path to repo-relative (forward slashes).
  const norm = absPath.replace(/\\/g, '/')
  const rootNorm = root.replace(/\\/g, '/').replace(/\/$/, '')
  const rel = norm.startsWith(rootNorm + '/') ? norm.slice(rootNorm.length + 1) : norm
  return state.files.find((f) => f.path === rel) ?? null
}
