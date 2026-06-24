import { writable, get } from 'svelte/store'

/**
 * Multi-terminal session state (Milestone 3, Wave 4 — GWEN-248).
 *
 * This store owns only the *list* of sessions and which one is active. The
 * actual PTY (spawned via `terminalCreate`) is owned by each TerminalInstance
 * component, which reports its backend id back here once created. Keeping the
 * list in a store (rather than inside the Terminal component) is what lets all
 * sessions stay alive across tab switches and panel collapse — switching only
 * flips `activeId`; nothing is unmounted.
 */
export interface TerminalSession {
  /** Stable client-side key (tab identity), independent of the PTY id. */
  key: string
  /** Display label shown on the tab, e.g. "Terminal 1". */
  title: string
  /**
   * Backend PTY session id from `terminalCreate`, or null until the instance
   * has spawned it. Used by the close handler to `terminalKill` the right PTY.
   */
  ptyId: string | null
  /**
   * Directory the shell should start in. `null` means "use the project folder"
   * (the default). Set by "Open in Terminal" (M9) to a specific folder.
   */
  cwd: string | null
}

export interface TerminalSessionsState {
  sessions: TerminalSession[]
  activeKey: string | null
}

const initial: TerminalSessionsState = { sessions: [], activeKey: null }

export const terminalSessions = writable<TerminalSessionsState>(initial)

function genKey(): string {
  return crypto.randomUUID
    ? crypto.randomUUID()
    : 'term-' + Date.now() + '-' + Math.random().toString(16).slice(2)
}

// Monotonic counter for the human-facing "Terminal N" label. Never reused, so
// closing tab 2 of [1,2,3] doesn't renumber the others.
let nextOrdinal = 1

/**
 * Adds a new session and makes it active. Returns its key. The PTY itself is
 * spawned lazily by the mounted TerminalInstance, which then calls
 * {@link bindPtyId}. Pass `cwd` to start the shell in a specific directory
 * (e.g. "Open in Terminal" on a folder); omit it to use the project folder.
 */
export function createSession(cwd: string | null = null): string {
  const key = genKey()
  const title = `Terminal ${nextOrdinal++}`
  terminalSessions.update((s) => ({
    sessions: [...s.sessions, { key, title, ptyId: null, cwd }],
    activeKey: key,
  }))
  return key
}

/** Records the backend PTY id for a session once its instance has spawned it. */
export function bindPtyId(key: string, ptyId: string): void {
  terminalSessions.update((s) => ({
    ...s,
    sessions: s.sessions.map((sess) =>
      sess.key === key ? { ...sess, ptyId } : sess
    ),
  }))
}

/** Switches the active session. */
export function activateSession(key: string): void {
  terminalSessions.update((s) => ({ ...s, activeKey: key }))
}

/**
 * Removes a session from the list and picks a new active one (the neighbour to
 * the left, else the new last). Returns the removed session's `ptyId` (if any)
 * so the caller can `terminalKill` it — the store stays free of Tauri calls.
 */
export function removeSession(key: string): string | null {
  const before = get(terminalSessions)
  const idx = before.sessions.findIndex((s) => s.key === key)
  const removed = idx >= 0 ? before.sessions[idx] : null

  terminalSessions.update((s) => {
    const sessions = s.sessions.filter((sess) => sess.key !== key)
    let activeKey = s.activeKey
    if (s.activeKey === key) {
      // Prefer the left neighbour, else whatever is now last, else none.
      const neighbour = sessions[idx - 1] ?? sessions[sessions.length - 1] ?? null
      activeKey = neighbour?.key ?? null
    }
    return { sessions, activeKey }
  })

  return removed?.ptyId ?? null
}

/** Ensures at least one session exists (called when the panel first opens). */
export function ensureInitialSession(): void {
  if (get(terminalSessions).sessions.length === 0) {
    createSession()
  }
}
