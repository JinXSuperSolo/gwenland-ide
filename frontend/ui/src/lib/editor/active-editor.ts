import { EditorView } from '@codemirror/view'
import {
  addCursorAbove,
  addCursorBelow,
  copyLineDown,
  copyLineUp,
  deleteLine,
  moveLineDown,
  moveLineUp,
  redo,
  toggleComment,
  undo,
} from '@codemirror/commands'
import {
  gotoLine,
  openSearchPanel,
  selectNextOccurrence,
  selectSelectionMatches,
} from '@codemirror/search'
import { lspPath, openSearchPanel as openCustomSearchPanel } from './codemirror-setup'
import { lspDefinition, type LspDefinitionLocation } from '../tauri/commands'
import { lspChangePath } from '../stores/lsp'

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

export function activeIndentInfo(): { kind: 'Spaces' | 'Tabs'; size: number } | null {
  if (!active) return null
  const lines = active.state.doc.toString().split('\n')
  let tabLines = 0
  const spaceCounts: number[] = []
  for (const line of lines) {
    if (!line.trim()) continue
    const match = line.match(/^(\s+)/)
    if (!match) continue
    const indent = match[1]
    if (indent.startsWith('\t')) tabLines += 1
    const spaces = indent.match(/^ +/)?.[0].length ?? 0
    if (spaces > 0) spaceCounts.push(spaces)
  }
  if (tabLines > spaceCounts.length) return { kind: 'Tabs', size: 1 }
  const size = spaceCounts.find((count) => count % 2 === 0 && count <= 8) ?? 2
  return { kind: 'Spaces', size: Math.max(2, Math.min(size, 8)) }
}

export function editorUndo(): void {
  runEditorCommand(undo)
}
export function editorRedo(): void {
  runEditorCommand(redo)
}
export function editorFind(): void {
  if (active) openCustomSearchPanel(active)
}
export function editorReplace(): void {
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

function runEditorCommand(command: (target: View) => boolean): void {
  if (!active) return
  command(active)
  active.focus()
}

export function editorToggleComment(): void {
  runEditorCommand(toggleComment)
}

export function editorMoveLineUp(): void {
  runEditorCommand(moveLineUp)
}

export function editorMoveLineDown(): void {
  runEditorCommand(moveLineDown)
}

export function editorCopyLineUp(): void {
  runEditorCommand(copyLineUp)
}

export function editorCopyLineDown(): void {
  runEditorCommand(copyLineDown)
}

export function editorDeleteLine(): void {
  runEditorCommand(deleteLine)
}

export function editorSelectNextMatch(): void {
  runEditorCommand(selectNextOccurrence)
}

export function editorSelectAllMatches(): void {
  runEditorCommand(selectSelectionMatches)
}

export function editorAddCursorAbove(): void {
  runEditorCommand(addCursorAbove)
}

export function editorAddCursorBelow(): void {
  runEditorCommand(addCursorBelow)
}

export function editorGoToLine(): void {
  runEditorCommand(gotoLine)
}

function pendingEditorFeature(name: string): void {
  console.info(`[GwenLand] "${name}" needs an LSP edit/navigation command that is not wired yet.`)
}

export function editorFormatDocument(): void {
  pendingEditorFeature('Format Document')
}

function samePath(a: string, b: string): boolean {
  const norm = (value: string) => value.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase()
  return norm(a) === norm(b)
}

function emitDefinitionNavigation(location: LspDefinitionLocation): void {
  window.dispatchEvent(new CustomEvent('gwenland:open-definition', { detail: location }))
}

export async function editorGoToDefinitionAt(
  path: string,
  line: number,
  character: number,
): Promise<void> {
  if (!path) return
  if (active && samePath(active.state.facet(lspPath), path)) {
    await lspChangePath(path, active.state.doc.toString())
  }
  const location = await lspDefinition(path, line, character, 0).catch(() => null)
  if (!location) return
  if (samePath(location.path, path) && active) {
    const target = active.state.doc.line(Math.min(location.line + 1, active.state.doc.lines))
    const pos = Math.min(target.from + location.character, target.to)
    active.dispatch({ selection: { anchor: pos }, scrollIntoView: true })
    active.focus()
    return
  }
  emitDefinitionNavigation(location)
}

export function editorGoToDefinition(): void {
  if (!active) return
  const path = active.state.facet(lspPath)
  if (!path) {
    pendingEditorFeature('Go to Definition')
    return
  }
  const pos = active.state.selection.main.head
  const line = active.state.doc.lineAt(pos)
  void editorGoToDefinitionAt(path, line.number - 1, pos - line.from)
}

export function editorRenameSymbol(): void {
  pendingEditorFeature('Rename Symbol')
}

export function editorFindWorkspace(): void {
  pendingEditorFeature('Find in Workspace')
}

export function editorSplit(): void {
  pendingEditorFeature('Split Editor')
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
