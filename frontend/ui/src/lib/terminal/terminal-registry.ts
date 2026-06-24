/**
 * Live xterm instance registry (Milestone 9). The terminal context-menu actions
 * are global, but Copy/Paste/Clear/Select All must act on the *specific* xterm
 * the menu was opened over. Each `TerminalInstance` registers a handle here
 * (keyed by its session key) on mount and unregisters on destroy — mirroring how
 * `active-editor.ts` exposes the live editor to menu/shortcut code without
 * threading refs through the component tree.
 */
export interface TerminalHandle {
  /** Currently selected text in this terminal (empty string if none). */
  getSelection(): string
  /** Copy the current selection to the clipboard. */
  copySelection(): Promise<void>
  /** Paste clipboard text into the PTY at the prompt. */
  paste(): Promise<void>
  /** Clear the viewport + scrollback. */
  clear(): void
  /** Select all buffered text. */
  selectAll(): void
  /** Focus the terminal. */
  focus(): void
  /** Last `maxLines` non-empty lines of the scrollback (GWEN-332 @terminal). */
  readBuffer(maxLines: number): string
}

/** Read the most recent `maxLines` lines from the active session's buffer, or
 *  '' when no terminal is registered. Trailing blank lines are trimmed. */
export function readActiveTerminalBuffer(
  key: string | undefined,
  maxLines: number
): string {
  return getTerminalHandle(key)?.readBuffer(maxLines) ?? ''
}

const handles = new Map<string, TerminalHandle>()

export function registerTerminalHandle(key: string, handle: TerminalHandle): void {
  handles.set(key, handle)
}

export function unregisterTerminalHandle(key: string): void {
  handles.delete(key)
}

export function getTerminalHandle(key: string | undefined): TerminalHandle | undefined {
  return key ? handles.get(key) : undefined
}
