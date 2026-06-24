import { EditorView } from '@codemirror/view'
import { undo, redo, openSearchPanel } from './codemirror-setup'

type View = EditorView

/**
 * A module-level handle to the currently-mounted EditorView. The Editor
 * component sets/clears this so menu bar, palette and shortcuts can act on the
 * live editor (undo/redo/find/save) without threading refs through the tree.
 * `null` when no tab is open.
 */
let active: View | null = null

export function setActiveEditor(view: View | null): void {
  active = view
}

export function hasActiveEditor(): boolean {
  return active !== null
}

/** Live document of the active editor, or null if none. */
export function activeDoc(): string | null {
  return active ? active.state.doc.toString() : null
}

/** The active editor's selected text, or null if none/empty (M4 attachments). */
export function activeSelection(): string | null {
  if (!active) return null
  const sel = active.state.selection.main
  if (sel.empty) return null
  return active.state.sliceDoc(sel.from, sel.to)
}

export function editorUndo(): void {
  if (active) {
    undo(active)
    active.focus()
  }
}
export function editorRedo(): void {
  if (active) {
    redo(active)
    active.focus()
  }
}
export function editorFind(): void {
  if (active) openSearchPanel(active)
}
export function editorSelectAll(): void {
  if (!active) return
  active.dispatch({ selection: { anchor: 0, head: active.state.doc.length } })
  active.focus()
}
export function focusEditor(): void {
  if (active) active.focus()
}

// --- Clipboard (Milestone 9 — editor context menu) -------------------------
// WebView2 supports the async Clipboard API (used elsewhere in the app). Each
// fails soft: no active editor / denied clipboard simply does nothing.

/** Copy the current selection to the clipboard. No-op when nothing is selected. */
export async function editorCopy(): Promise<void> {
  const text = activeSelection()
  if (!text) return
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    /* clipboard unavailable */
  }
  active?.focus()
}

/** Copy the selection, then delete it (single undo step). No-op when empty. */
export async function editorCut(): Promise<void> {
  if (!active) return
  const sel = active.state.selection.main
  if (sel.empty) return
  const text = active.state.sliceDoc(sel.from, sel.to)
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    return // don't delete if we couldn't copy
  }
  active.dispatch({
    changes: { from: sel.from, to: sel.to, insert: '' },
    selection: { anchor: sel.from },
  })
  active.focus()
}

/** Paste clipboard text at the cursor (replacing any selection). */
export async function editorPaste(): Promise<void> {
  if (!active) return
  let text = ''
  try {
    text = await navigator.clipboard.readText()
  } catch {
    return // clipboard read denied/unavailable
  }
  if (!text) return
  insertAtCursor(text)
}

/**
 * Replace the entire active document (diff review apply). Uses a single change
 * so the edit is one undo step and existing undo history/user edits are kept.
 * Returns false when no editor is active.
 */
export function replaceActiveDocument(text: string): boolean {
  if (!active) return false
  active.dispatch({ changes: { from: 0, to: active.state.doc.length, insert: text } })
  return true
}

/** Scroll the active editor to a 1-based line (diff review navigation). */
export function revealLine(lineNo: number): void {
  if (!active) return
  const n = Math.min(Math.max(lineNo, 1), active.state.doc.lines)
  const pos = active.state.doc.line(n).from
  active.dispatch({ selection: { anchor: pos }, scrollIntoView: true })
}

/**
 * Select a 1-based line range in the active editor and scroll it into view
 * (GWEN-332 — clicking an @mention pill jumps to its source lines). Clamps to
 * the document bounds; a single line selects just that line. No-op when no
 * editor is active.
 */
export function selectRange(startLine: number, endLine: number): void {
  if (!active) return
  const lines = active.state.doc.lines
  const a = Math.min(Math.max(startLine, 1), lines)
  const b = Math.min(Math.max(endLine, a), lines)
  const from = active.state.doc.line(a).from
  const to = active.state.doc.line(b).to
  active.dispatch({ selection: { anchor: from, head: to }, scrollIntoView: true })
  active.focus()
}

/**
 * Insert `text` at the active editor's cursor (replacing any selection), then
 * place the caret after it and focus. Returns false when no editor is active
 * (M4 AiCodeBlock "Insert into Editor"). The caller passes raw code including
 * any trailing newline, which is preserved verbatim.
 */
export function insertAtCursor(text: string): boolean {
  if (!active) return false
  const sel = active.state.selection.main
  active.dispatch({
    changes: { from: sel.from, to: sel.to, insert: text },
    selection: { anchor: sel.from + text.length },
  })
  active.focus()
  return true
}
