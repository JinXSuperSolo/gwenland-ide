import { get } from 'svelte/store'
import { terminalSessions } from '../stores/terminal-sessions'
import { terminalWrite } from '../tauri/commands'

/**
 * GWEN-325: terminal ↔ workspace CWD sync helpers. Kept out of the
 * `terminal-sessions` store so that store stays Tauri-free.
 */

const encoder = new TextEncoder()

/** Quote a path for a shell `cd`, handling spaces uniformly on all platforms. */
function quote(path: string): string {
  return `"${path.replace(/"/g, '\\"')}"`
}

/**
 * Silently change-directory every live terminal session to `path`. Called when a
 * folder is opened while terminals are already running, so existing shells follow
 * the workspace (Requirement: "auto-cd silently"). Sessions whose PTY hasn't
 * spawned yet are skipped — they pick up the new folder as their spawn cwd.
 */
export function autoCdSessions(path: string): void {
  const { sessions } = get(terminalSessions)
  for (const session of sessions) {
    // Only sessions without an explicit cwd ("Open in Terminal" pins its own).
    if (!session.ptyId || session.cwd) continue
    const line = `cd ${quote(path)}\r`
    void terminalWrite(session.ptyId, encoder.encode(line)).catch(() => {})
  }
}
