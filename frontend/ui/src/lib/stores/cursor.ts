import { writable } from 'svelte/store'
import type { EditorState } from '@codemirror/state'

/**
 * The active editor's primary cursor position, mirrored out of CodeMirror (which
 * lives outside Svelte reactivity) so the status bar can show Ln/Col reactively.
 * 1-based, matching editor convention.
 */
export interface CursorPos {
  line: number
  col: number
}

/** Null when no editor is active (no tab open). */
export const cursor = writable<CursorPos | null>(null)

/** Compute 1-based Ln/Col of the primary selection head from an EditorState. */
export function posFromState(state: EditorState): CursorPos {
  const head = state.selection.main.head
  const line = state.doc.lineAt(head)
  return { line: line.number, col: head - line.from + 1 }
}

/** Push the active editor's cursor position into the store. */
export function setCursorFromState(state: EditorState): void {
  cursor.set(posFromState(state))
}

/** Clear the cursor (e.g. when the last tab closes). */
export function clearCursor(): void {
  cursor.set(null)
}
