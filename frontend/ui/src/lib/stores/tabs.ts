import { writable, get } from 'svelte/store'
import type { EditorState } from '@codemirror/state'
import { createEditorState } from '../editor/codemirror-setup'
import { activeDoc } from '../editor/active-editor'
import { readFile, writeFile } from '../tauri/commands'
import { lspOpenPath, lspClosePath } from './lsp'

/**
 * The center workspace area is tab-based, and tabs are no longer all editors:
 * a tab is a discriminated union on `kind` so non-editor surfaces (the M5 web
 * preview) can live alongside file editors in the same tab strip. Shared fields
 * (`id`, `name`) live on every tab; kind-specific state hangs off the variant.
 * Narrow with [`isEditorTab`] / [`isPreviewTab`] before touching variant fields.
 */
export type TabKind = 'editor' | 'preview'

/**
 * Where a preview tab points (mirrors the engine's `PreviewSource`, M5):
 * a local static file loaded by path, or a running dev server by URL.
 */
export type PreviewSource =
  | { kind: 'static-file'; path: string }
  | { kind: 'dev-server'; url: string; port: number }

interface TabCommon {
  id: string
  /** Label shown on the tab. */
  name: string
}

/**
 * A file editor tab. Each owns its own CodeMirror EditorState so cursor, scroll
 * and undo history never bleed across tabs when switching.
 *
 * Dirty tracking (GWEN-240): a tab is dirty iff its live document differs from
 * `baseline` (the last-saved content) — NOT merely because an edit event fired.
 * Opening a file therefore never marks it dirty, and reverting an edit clears
 * the dot. This is the bug-fixed semantics from the legacy version.
 */
export interface EditorTab extends TabCommon {
  kind: 'editor'
  path: string
  /** Last-saved content. dirty := liveDoc !== baseline. */
  baseline: string
  /** Per-tab CM6 state, snapshotted on switch and restored on activate. */
  state: EditorState
  dirty: boolean
}

/** A web-preview tab (M5). Stateless beyond the source it renders; never dirty. */
export interface PreviewTab extends TabCommon {
  kind: 'preview'
  source: PreviewSource
}

export type Tab = EditorTab | PreviewTab

/** Narrowing guard: true (and refines the type) for file-editor tabs. */
export function isEditorTab(tab: Tab): tab is EditorTab {
  return tab.kind === 'editor'
}

/** Narrowing guard: true (and refines the type) for web-preview tabs. */
export function isPreviewTab(tab: Tab): tab is PreviewTab {
  return tab.kind === 'preview'
}

export interface TabsState {
  tabs: Tab[]
  activeId: string | null
}

const initial: TabsState = { tabs: [], activeId: null }

export const tabs = writable<TabsState>(initial)

function genId(): string {
  return crypto.randomUUID
    ? crypto.randomUUID()
    : 'tab-' + Date.now() + '-' + Math.random().toString(16).slice(2)
}

function basename(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() || path
}

/** The engine's binary-file error message (engine/src/fs.rs). */
const BINARY_FILE = 'binary file'

export interface OpenFileResult {
  ok: boolean
  /** Set when ok=false; a human-readable reason (e.g. binary file). */
  error?: string
}

/**
 * Open `filePath` in a tab. If already open, just activates that tab (dedup).
 * Reads content via the existing read_file command; the loaded content becomes
 * the saved baseline (so a freshly-opened file is never dirty).
 */
export async function openFile(filePath: string): Promise<OpenFileResult> {
  const existing = get(tabs).tabs.find((t) => isEditorTab(t) && t.path === filePath)
  if (existing) {
    activateTab(existing.id)
    return { ok: true }
  }

  let content: string
  try {
    content = await readFile(filePath)
  } catch (e) {
    const msg = String(e)
    if (msg.toLowerCase().includes(BINARY_FILE)) {
      return { ok: false, error: 'Binary file — cannot open in editor' }
    }
    return { ok: false, error: 'Could not open file: ' + msg }
  }

  const id = genId()
  const tab: EditorTab = {
    id,
    kind: 'editor',
    path: filePath,
    name: basename(filePath),
    baseline: content,
    state: createEditorState(content),
    dirty: false,
  }
  tabs.update((s) => ({ tabs: [...s.tabs, tab], activeId: id }))
  // Fire-and-forget: connect the language server in the background so opening a
  // file is never blocked by server startup (Requirement 12.6).
  void lspOpenPath(filePath, content)
  return { ok: true }
}

/**
 * Create a new empty untitled tab (in-memory scratch buffer). It has no disk
 * path yet; an empty baseline means it starts clean and goes dirty once typed.
 * (Saving an untitled buffer to a chosen path is a later enhancement.)
 */
export function newUntitledFile(): void {
  const id = genId()
  const n = get(tabs).tabs.filter((t) => isEditorTab(t) && t.path === '').length + 1
  const tab: EditorTab = {
    id,
    kind: 'editor',
    path: '',
    name: n === 1 ? 'Untitled' : `Untitled-${n}`,
    baseline: '',
    state: createEditorState(''),
    dirty: false,
  }
  tabs.update((s) => ({ tabs: [...s.tabs, tab], activeId: id }))
}

/** A preview source's dedup key: static files by path, dev servers by URL. */
function previewKey(source: PreviewSource): string {
  return source.kind === 'static-file' ? source.path : source.url
}

/** A preview tab's label from its source. */
function previewName(source: PreviewSource): string {
  return source.kind === 'static-file'
    ? `Preview: ${basename(source.path)}`
    : `Preview: localhost:${source.port}`
}

/**
 * Open (or focus, if already open) a web-preview tab for `source` (M5). Dedup is
 * by source key (file path / server URL); re-opening an existing preview updates
 * its source in place — e.g. a dev server that restarted on a new port — so the
 * pane reloads rather than spawning a duplicate tab. Returns the tab id.
 */
export function openPreview(source: PreviewSource): string {
  const key = previewKey(source)
  const existing = get(tabs).tabs.find((t) => isPreviewTab(t) && previewKey(t.source) === key)
  if (existing) {
    tabs.update((s) => ({
      ...s,
      activeId: existing.id,
      tabs: s.tabs.map((t) =>
        t.id === existing.id && isPreviewTab(t) ? { ...t, source, name: previewName(source) } : t,
      ),
    }))
    return existing.id
  }
  const id = genId()
  const tab: PreviewTab = { id, kind: 'preview', name: previewName(source), source }
  tabs.update((s) => ({ tabs: [...s.tabs, tab], activeId: id }))
  return id
}

/** Switch the active tab. No-op if already active. */
export function activateTab(id: string): void {
  tabs.update((s) => (s.activeId === id ? s : { ...s, activeId: id }))
}

/**
 * Snapshot a tab's live EditorState back into the store (called by the Editor
 * component before it tears down a view, so the next activate restores exactly
 * what the user was looking at). Recomputes dirty against the baseline.
 */
export function persistTabState(id: string, state: EditorState): void {
  tabs.update((s) => ({
    ...s,
    tabs: s.tabs.map((t) =>
      t.id === id && isEditorTab(t)
        ? { ...t, state, dirty: state.doc.toString() !== t.baseline }
        : t,
    ),
  }))
}

/**
 * Recompute a tab's dirty flag from its live document (called on every edit).
 * Cheap: only writes the store when the dirty bit actually flips.
 */
export function recomputeDirty(id: string, currentDoc: string): void {
  tabs.update((s) => {
    const tab = s.tabs.find((t) => t.id === id)
    if (!tab || !isEditorTab(tab)) return s
    const nextDirty = currentDoc !== tab.baseline
    if (nextDirty === tab.dirty) return s
    return {
      ...s,
      tabs: s.tabs.map((t) => (t.id === id && isEditorTab(t) ? { ...t, dirty: nextDirty } : t)),
    }
  })
}

/**
 * Save a tab to disk via the atomic write_file. `currentContent` is the live
 * document captured by the caller up front (so the write targets the right
 * content even if the active tab changes during the await). On success the
 * baseline moves forward and the tab is no longer dirty.
 */
export async function saveTab(id: string, currentContent: string): Promise<OpenFileResult> {
  const tab = get(tabs).tabs.find((t) => t.id === id)
  if (!tab || !isEditorTab(tab)) return { ok: false, error: 'tab not found' }
  try {
    await writeFile(tab.path, currentContent)
  } catch (e) {
    return { ok: false, error: 'Save failed: ' + String(e) }
  }
  // Only update if the tab still exists after the await.
  tabs.update((s) => ({
    ...s,
    tabs: s.tabs.map((t) =>
      t.id === id && isEditorTab(t) ? { ...t, baseline: currentContent, dirty: false } : t,
    ),
  }))
  return { ok: true }
}

/**
 * Replace a (non-active) editor tab's content after a diff-review apply wrote it
 * to disk: rebuild its stored state and move the baseline forward so it's clean.
 * The active tab is updated through the live view instead (see active-editor).
 */
export function setTabContent(id: string, text: string): void {
  tabs.update((s) => ({
    ...s,
    tabs: s.tabs.map((t) =>
      t.id === id && isEditorTab(t)
        ? { ...t, state: createEditorState(text, undefined, undefined, t.path), baseline: text, dirty: false }
        : t
    ),
  }))
}

/** Save the active tab using the live editor document. No-op if no active tab. */
export async function saveActiveTab(): Promise<void> {
  const id = get(tabs).activeId
  if (!id) return
  // Only editor tabs are saveable; a preview tab has nothing to write.
  const tab = get(tabs).tabs.find((t) => t.id === id)
  if (!tab || !isEditorTab(tab)) return
  const content = activeDoc()
  if (content === null) return
  const res = await saveTab(id, content)
  if (!res.ok) console.error(res.error)
}

/**
 * Close the active tab, prompting (native confirm) when it has unsaved changes.
 * Used by Ctrl+W / the File menu's Close Editor.
 */
export function closeActiveTab(): void {
  const id = get(tabs).activeId
  if (!id) return
  const tab = get(tabs).tabs.find((t) => t.id === id)
  if (tab && isEditorTab(tab) && tab.dirty) {
    const ok = confirm(`"${tab.name}" has unsaved changes. Close without saving?`)
    if (!ok) return
  }
  closeTab(id)
}

/** Confirm before discarding a set of dirty tabs (single prompt for the batch).
 *  Returns true to proceed. Non-editor/clean tabs never block. */
function confirmCloseDirty(list: Tab[]): boolean {
  const dirty = list.filter((t) => isEditorTab(t) && t.dirty)
  if (dirty.length === 0) return true
  const names = dirty.map((t) => t.name).join(', ')
  return confirm(
    `${dirty.length} unsaved file(s) will be closed without saving:\n${names}\n\nContinue?`,
  )
}

/** Close a specific tab (context menu "Close"), prompting if it's dirty. */
export function closeTabById(id: string): void {
  const tab = get(tabs).tabs.find((t) => t.id === id)
  if (!tab) return
  if (isEditorTab(tab) && tab.dirty) {
    if (!confirm(`"${tab.name}" has unsaved changes. Close without saving?`)) return
  }
  closeTab(id)
}

/** Close every tab except `keepId` (context menu "Close Others"). */
export function closeOtherTabs(keepId: string): void {
  const others = get(tabs).tabs.filter((t) => t.id !== keepId)
  if (!confirmCloseDirty(others)) return
  for (const t of others) closeTab(t.id)
}

/** Close all tabs to the right of `fromId` (context menu "Close to Right"). */
export function closeTabsToRight(fromId: string): void {
  const s = get(tabs)
  const idx = s.tabs.findIndex((t) => t.id === fromId)
  if (idx === -1) return
  const toClose = s.tabs.slice(idx + 1)
  if (!confirmCloseDirty(toClose)) return
  for (const t of toClose) closeTab(t.id)
}

/** Close all non-dirty tabs (context menu "Close Saved"). Preview tabs count
 *  as saved (never dirty). */
export function closeSavedTabs(): void {
  const saved = get(tabs).tabs.filter((t) => !(isEditorTab(t) && t.dirty))
  for (const t of saved) closeTab(t.id)
}

/** Cycle the active tab by `dir` (+1 next, -1 prev), wrapping around. */
export function cycleTab(dir: number): void {
  const s = get(tabs)
  if (s.tabs.length < 2) return
  const i = s.tabs.findIndex((t) => t.id === s.activeId)
  if (i === -1) return
  const next = (i + dir + s.tabs.length) % s.tabs.length
  activateTab(s.tabs[next].id)
}

/**
 * Close a tab. When closing the active tab, activates the next tab (or the
 * previous if it was last). Returns the now-active id (or null if none left).
 * Dirty confirmation is the caller's responsibility.
 */
export function closeTab(id: string): void {
  // Send didClose + clear LSP state for an editor tab being closed
  // (Requirement 9.9/10.10). Done before the store update so the path is known.
  const closing = get(tabs).tabs.find((t) => t.id === id)
  if (closing && isEditorTab(closing) && closing.path) {
    void lspClosePath(closing.path)
  }
  tabs.update((s) => {
    const idx = s.tabs.findIndex((t) => t.id === id)
    if (idx === -1) return s
    const remaining = s.tabs.filter((t) => t.id !== id)
    let activeId = s.activeId
    if (s.activeId === id) {
      const next = remaining[idx] ?? remaining[idx - 1] ?? null
      activeId = next ? next.id : null
    }
    return { tabs: remaining, activeId }
  })
}
