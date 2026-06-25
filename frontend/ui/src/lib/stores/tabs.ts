import { writable, get } from 'svelte/store'
import type { EditorState } from '@codemirror/state'
import { createEditorState } from '../editor/codemirror-setup'
import { activeDoc } from '../editor/active-editor'
import { readFile, writeFile } from '../tauri/commands'
import { lspOpenPath, lspClosePath } from './lsp'
import { openPrompt } from './prompt-dialog'
import { workspace } from './workspace'
import { scheduleHistorySnapshot } from './history-snapshots'

export type TabKind = 'editor' | 'preview' | 'diff'
export type EditorGroupOrientation = 'horizontal' | 'vertical'

export type PreviewSource =
  | { kind: 'static-file'; path: string }
  | { kind: 'dev-server'; url: string; port: number }

interface TabCommon {
  id: string
  name: string
  /** Temporary preview tab. Replaced by the next single-click preview open. */
  preview?: boolean
}

export interface EditorTab extends TabCommon {
  kind: 'editor'
  path: string
  baseline: string
  state: EditorState
  dirty: boolean
}

export interface PreviewTab extends TabCommon {
  kind: 'preview'
  source: PreviewSource
}

export interface DiffTab extends TabCommon {
  kind: 'diff'
  path: string
  root: string
  untracked: boolean
}

export type Tab = EditorTab | PreviewTab | DiffTab

export interface EditorGroup {
  id: string
  tabs: Tab[]
  activeId: string | null
  isLocked: boolean
  isMaximized: boolean
  /** Relative flex size. Kept unitless so row/column layouts can share it. */
  size: number
}

export interface TabsState {
  /** Flattened compatibility view for older consumers. */
  tabs: Tab[]
  /** Active tab in the active group, also for older consumers. */
  activeId: string | null
  groups: EditorGroup[]
  activeGroupId: string
  orientation: EditorGroupOrientation
}

export interface OpenFileOptions {
  groupId?: string
  preview?: boolean
  ignoreLock?: boolean
}

export interface OpenPreviewOptions {
  groupId?: string
  preview?: boolean
  ignoreLock?: boolean
}

export interface OpenFileResult {
  ok: boolean
  error?: string
}

export interface PersistedEditorGroupTab {
  path: string
  type: string
  isDirty: boolean
  isPreview?: boolean
}

export interface PersistedEditorGroup {
  id: string
  tabs: PersistedEditorGroupTab[]
  activeTabPath: string
  isLocked: boolean
  isMaximized: boolean
  size?: number
}

const ROOT_GROUP_ID = 'group-root'
const IMAGE_EXTS = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'ico', 'bmp'])
const BINARY_FILE = 'binary file'

function genId(prefix = 'tab'): string {
  return crypto.randomUUID
    ? `${prefix}-${crypto.randomUUID()}`
    : `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`
}

function basename(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() || path
}

function sep(path: string): string {
  return path.includes('\\') ? '\\' : '/'
}

function joinPath(parent: string, child: string): string {
  if (!parent) return child
  if (/^[a-zA-Z]:[\\/]/.test(child) || child.startsWith('/') || child.startsWith('\\')) {
    return child
  }
  const s = sep(parent)
  return parent.endsWith(s) ? parent + child : parent + s + child
}

function makeGroup(partial: Partial<EditorGroup> = {}): EditorGroup {
  const tabs = partial.tabs ?? []
  const activeId =
    partial.activeId && tabs.some((tab) => tab.id === partial.activeId)
      ? partial.activeId
      : tabs.at(-1)?.id ?? null
  return {
    id: partial.id ?? genId('group'),
    tabs,
    activeId,
    isLocked: partial.isLocked ?? false,
    isMaximized: partial.isMaximized ?? false,
    size: Number.isFinite(partial.size) && partial.size! > 0 ? partial.size! : 1,
  }
}

function flatten(groups: EditorGroup[]): Tab[] {
  return groups.flatMap((group) => group.tabs)
}

function normalizeGroups(groups: EditorGroup[]): EditorGroup[] {
  const source = groups.length > 0 ? groups : [makeGroup({ id: ROOT_GROUP_ID })]
  return source.map((group) => makeGroup(group))
}

function derive(state: Partial<TabsState>): TabsState {
  let groups = normalizeGroups(state.groups ?? [])
  if (groups.length === 0) groups = [makeGroup({ id: ROOT_GROUP_ID })]
  const activeGroupId = groups.some((group) => group.id === state.activeGroupId)
    ? state.activeGroupId!
    : groups.find((group) => group.activeId)?.id ?? groups[0].id
  const activeGroup = groups.find((group) => group.id === activeGroupId) ?? groups[0]
  return {
    groups,
    activeGroupId,
    orientation: state.orientation ?? 'horizontal',
    tabs: flatten(groups),
    activeId: activeGroup.activeId,
  }
}

const initial = derive({
  groups: [makeGroup({ id: ROOT_GROUP_ID })],
  activeGroupId: ROOT_GROUP_ID,
  orientation: 'horizontal',
})

export const tabs = writable<TabsState>(initial)

function updateTabs(fn: (state: TabsState) => Partial<TabsState>): void {
  tabs.update((state) => derive(fn(derive(state))))
}

function groupForTab(state: TabsState, tabId: string): EditorGroup | undefined {
  return state.groups.find((group) => group.tabs.some((tab) => tab.id === tabId))
}

function activeGroup(state: TabsState): EditorGroup {
  return state.groups.find((group) => group.id === state.activeGroupId) ?? state.groups[0]
}

function previewKey(source: PreviewSource): string {
  return source.kind === 'static-file' ? source.path : source.url
}

function previewName(source: PreviewSource): string {
  return source.kind === 'static-file'
    ? `Preview: ${basename(source.path)}`
    : `Preview: localhost:${source.port}`
}

function tabRestoreKey(tab: Tab): string {
  if (isEditorTab(tab)) return tab.path
  if (isPreviewTab(tab)) return tab.source.kind === 'static-file' ? tab.source.path : tab.source.url
  if (isDiffTab(tab)) return tab.path
  return ''
}

function createEditorTab(path: string, content: string, preview = false): EditorTab {
  return {
    id: genId(),
    kind: 'editor',
    path,
    name: path ? basename(path) : 'Untitled',
    baseline: content,
    state: createEditorState(content, undefined, undefined, path),
    dirty: false,
    preview,
  }
}

function cloneTab(tab: Tab): Tab {
  if (isEditorTab(tab)) {
    const doc = tab.id === get(tabs).activeId ? activeDoc() ?? tab.state.doc.toString() : tab.state.doc.toString()
    return {
      ...tab,
      id: genId(),
      state: createEditorState(doc, undefined, undefined, tab.path),
      dirty: doc !== tab.baseline,
      preview: false,
    }
  }
  if (isPreviewTab(tab)) return { ...tab, id: genId(), preview: false }
  return { ...tab, id: genId(), preview: false }
}

function ensureWritableGroupId(preferred?: string, ignoreLock = false): string {
  const state = get(tabs)
  const wanted =
    (preferred && state.groups.find((group) => group.id === preferred)) ||
    activeGroup(state)
  if (wanted && (ignoreLock || !wanted.isLocked)) return wanted.id
  const unlocked = state.groups.find((group) => !group.isLocked)
  if (unlocked) return unlocked.id
  const id = genId('group')
  updateTabs((s) => ({
    ...s,
    groups: [...s.groups, makeGroup({ id })],
    activeGroupId: id,
  }))
  return id
}

function mapGroup(
  state: TabsState,
  groupId: string,
  mapper: (group: EditorGroup) => EditorGroup,
): EditorGroup[] {
  return state.groups.map((group) => (group.id === groupId ? makeGroup(mapper(group)) : group))
}

function closeLspIfLast(path: string, remaining: Tab[]): void {
  if (!path) return
  const stillOpen = remaining.some((tab) => isEditorTab(tab) && tab.path === path)
  if (!stillOpen) void lspClosePath(path)
}

export function isEditorTab(tab: Tab): tab is EditorTab {
  return tab.kind === 'editor'
}

export function isPreviewTab(tab: Tab): tab is PreviewTab {
  return tab.kind === 'preview'
}

export function isDiffTab(tab: Tab): tab is DiffTab {
  return tab.kind === 'diff'
}

export function isImagePath(path: string): boolean {
  const ext = path.split('.').pop()?.toLowerCase() ?? ''
  return IMAGE_EXTS.has(ext)
}

export function setActiveGroup(groupId: string): void {
  updateTabs((state) => ({
    ...state,
    activeGroupId: state.groups.some((group) => group.id === groupId)
      ? groupId
      : state.activeGroupId,
  }))
}

export function activateTab(id: string, groupId?: string): void {
  updateTabs((state) => {
    const group = groupId
      ? state.groups.find((candidate) => candidate.id === groupId)
      : groupForTab(state, id)
    if (!group || !group.tabs.some((tab) => tab.id === id)) return state
    return {
      ...state,
      activeGroupId: group.id,
      groups: mapGroup(state, group.id, (g) => ({ ...g, activeId: id })),
    }
  })
}

export async function openFile(
  filePath: string,
  options: OpenFileOptions = {},
): Promise<OpenFileResult> {
  if (isImagePath(filePath)) {
    openPreview({ kind: 'static-file', path: filePath }, options)
    return { ok: true }
  }

  const groupId = ensureWritableGroupId(options.groupId, options.ignoreLock)
  const existing = get(tabs)
    .groups.find((group) => group.id === groupId)
    ?.tabs.find((tab) => isEditorTab(tab) && tab.path === filePath)
  if (existing) {
    updateTabs((state) => ({
      ...state,
      activeGroupId: groupId,
      groups: mapGroup(state, groupId, (group) => ({
        ...group,
        activeId: existing.id,
        tabs: group.tabs.map((tab) =>
          tab.id === existing.id && isEditorTab(tab) && !options.preview
            ? { ...tab, preview: false }
            : tab,
        ),
      })),
    }))
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

  const tab = createEditorTab(filePath, content, !!options.preview)
  updateTabs((state) => ({
    ...state,
    activeGroupId: groupId,
    groups: mapGroup(state, groupId, (group) => {
      let nextTabs = group.tabs
      if (options.preview) {
        const preview = group.tabs.find((candidate) => candidate.preview)
        if (preview) {
          if (isEditorTab(preview)) closeLspIfLast(preview.path, state.tabs.filter((t) => t.id !== preview.id))
          nextTabs = group.tabs.map((candidate) => (candidate.id === preview.id ? { ...tab, id: preview.id } : candidate))
          return { ...group, tabs: nextTabs, activeId: preview.id }
        }
      }
      nextTabs = [...group.tabs, tab]
      return { ...group, tabs: nextTabs, activeId: tab.id }
    }),
  }))
  void lspOpenPath(filePath, content)
  return { ok: true }
}

export function newUntitledFile(): void {
  const groupId = ensureWritableGroupId()
  const state = get(tabs)
  const n = state.tabs.filter((tab) => isEditorTab(tab) && tab.path === '').length + 1
  const tab: EditorTab = {
    ...createEditorTab('', ''),
    name: n === 1 ? 'Untitled' : `Untitled-${n}`,
  }
  updateTabs((s) => ({
    ...s,
    activeGroupId: groupId,
    groups: mapGroup(s, groupId, (group) => ({ ...group, tabs: [...group.tabs, tab], activeId: tab.id })),
  }))
}

export function openPreview(source: PreviewSource, options: OpenPreviewOptions = {}): string {
  const groupId = ensureWritableGroupId(options.groupId, options.ignoreLock)
  const key = previewKey(source)
  const existing = get(tabs)
    .groups.find((group) => group.id === groupId)
    ?.tabs.find((tab) => isPreviewTab(tab) && previewKey(tab.source) === key)
  if (existing) {
    updateTabs((state) => ({
      ...state,
      activeGroupId: groupId,
      groups: mapGroup(state, groupId, (group) => ({
        ...group,
        activeId: existing.id,
        tabs: group.tabs.map((tab) =>
          tab.id === existing.id && isPreviewTab(tab)
            ? { ...tab, source, name: previewName(source), preview: options.preview ?? tab.preview }
            : tab,
        ),
      })),
    }))
    return existing.id
  }

  const id = genId()
  const tab: PreviewTab = {
    id,
    kind: 'preview',
    name: previewName(source),
    source,
    preview: !!options.preview,
  }
  updateTabs((state) => ({
    ...state,
    activeGroupId: groupId,
    groups: mapGroup(state, groupId, (group) => {
      if (options.preview) {
        const preview = group.tabs.find((candidate) => candidate.preview)
        if (preview) {
          if (isEditorTab(preview)) closeLspIfLast(preview.path, state.tabs.filter((t) => t.id !== preview.id))
          return {
            ...group,
            tabs: group.tabs.map((candidate) => (candidate.id === preview.id ? { ...tab, id: preview.id } : candidate)),
            activeId: preview.id,
          }
        }
      }
      return { ...group, tabs: [...group.tabs, tab], activeId: id }
    }),
  }))
  return id
}

export function openDiff(
  root: string,
  path: string,
  untracked: boolean,
  groupId?: string,
  ignoreLock = false,
): string {
  const targetGroupId = ensureWritableGroupId(groupId, ignoreLock)
  const existing = get(tabs)
    .groups.find((group) => group.id === targetGroupId)
    ?.tabs.find((tab) => isDiffTab(tab) && tab.root === root && tab.path === path)
  if (existing) {
    activateTab(existing.id, targetGroupId)
    return existing.id
  }
  const id = genId()
  const tab: DiffTab = {
    id,
    kind: 'diff',
    path,
    root,
    untracked,
    name: `${basename(path)} (diff)`,
  }
  updateTabs((state) => ({
    ...state,
    activeGroupId: targetGroupId,
    groups: mapGroup(state, targetGroupId, (group) => ({ ...group, tabs: [...group.tabs, tab], activeId: id })),
  }))
  return id
}

export function persistTabState(id: string, state: EditorState): void {
  updateTabs((s) => ({
    ...s,
    groups: s.groups.map((group) =>
      makeGroup({
        ...group,
        tabs: group.tabs.map((tab) =>
          tab.id === id && isEditorTab(tab)
            ? { ...tab, state, dirty: state.doc.toString() !== tab.baseline }
            : tab,
        ),
      }),
    ),
  }))
}

export function recomputeDirty(id: string, currentDoc: string): void {
  const tab = get(tabs).tabs.find((candidate) => candidate.id === id)
  if (!tab || !isEditorTab(tab)) return
  const nextDirty = currentDoc !== tab.baseline
  if (nextDirty === tab.dirty) return
  updateTabs((s) => ({
    ...s,
    groups: s.groups.map((group) =>
      makeGroup({
        ...group,
        tabs: group.tabs.map((candidate) =>
          candidate.id === id && isEditorTab(candidate)
            ? { ...candidate, dirty: nextDirty }
            : candidate,
        ),
      }),
    ),
  }))
}

export async function saveTab(id: string, currentContent: string): Promise<OpenFileResult> {
  const tab = get(tabs).tabs.find((candidate) => candidate.id === id)
  if (!tab || !isEditorTab(tab)) return { ok: false, error: 'tab not found' }
  if (!tab.path) return { ok: false, error: 'Use Save As for untitled files' }
  try {
    await writeFile(tab.path, currentContent)
  } catch (e) {
    return { ok: false, error: 'Save failed: ' + String(e) }
  }
  updateTabs((s) => ({
    ...s,
    groups: s.groups.map((group) =>
      makeGroup({
        ...group,
        tabs: group.tabs.map((candidate) =>
          candidate.id === id && isEditorTab(candidate)
            ? { ...candidate, baseline: currentContent, dirty: false, preview: false }
            : candidate,
        ),
      }),
    ),
  }))
  scheduleHistorySnapshot(tab.path, currentContent, 'save')
  return { ok: true }
}

export function setTabContent(id: string, text: string): void {
  updateTabs((s) => ({
    ...s,
    groups: s.groups.map((group) =>
      makeGroup({
        ...group,
        tabs: group.tabs.map((tab) =>
          tab.id === id && isEditorTab(tab)
            ? {
                ...tab,
                state: createEditorState(text, undefined, undefined, tab.path),
                baseline: text,
                dirty: false,
                preview: false,
              }
            : tab,
        ),
      }),
    ),
  }))
}

export async function saveActiveTab(): Promise<void> {
  const id = get(tabs).activeId
  if (!id) return
  const tab = get(tabs).tabs.find((candidate) => candidate.id === id)
  if (!tab || !isEditorTab(tab)) return
  const content = activeDoc()
  if (content === null) return
  const res = await saveTab(id, content)
  if (!res.ok) console.error(res.error)
}

export async function saveActiveTabAs(): Promise<void> {
  const s = get(tabs)
  const id = s.activeId
  if (!id) return
  const tab = s.tabs.find((candidate) => candidate.id === id)
  if (!tab || !isEditorTab(tab)) return
  const content = activeDoc()
  if (content === null) return
  const root = get(workspace).folderPath ?? ''
  const name = await openPrompt({
    title: 'Save As',
    label: root ? 'File path relative to workspace' : 'Absolute file path',
    initialValue: tab.path ? basename(tab.path) : tab.name,
    placeholder: root ? 'src/example.ts' : 'C:\\path\\example.ts',
    confirmLabel: 'Save',
  })
  if (!name) return
  const target = root ? joinPath(root, name) : name
  try {
    await writeFile(target, content)
  } catch (e) {
    console.error('Save As failed:', e)
    return
  }
  updateTabs((state) => ({
    ...state,
    groups: state.groups.map((group) =>
      makeGroup({
        ...group,
        tabs: group.tabs.map((candidate) =>
          candidate.id === id && isEditorTab(candidate)
            ? {
                ...candidate,
                path: target,
                name: basename(target),
                baseline: content,
                state: createEditorState(content, undefined, undefined, target),
                dirty: false,
                preview: false,
              }
            : candidate,
        ),
      }),
    ),
  }))
  void lspOpenPath(target, content)
}

function confirmCloseDirty(list: Tab[]): boolean {
  const dirty = list.filter((tab) => isEditorTab(tab) && tab.dirty)
  if (dirty.length === 0) return true
  const names = dirty.map((tab) => tab.name).join(', ')
  return confirm(`${dirty.length} unsaved file(s) will be closed without saving:\n${names}\n\nContinue?`)
}

export function closeActiveTab(): void {
  const id = get(tabs).activeId
  if (!id) return
  const tab = get(tabs).tabs.find((candidate) => candidate.id === id)
  if (tab && isEditorTab(tab) && tab.dirty && !confirm(`"${tab.name}" has unsaved changes. Close without saving?`)) return
  closeTab(id)
}

export function closeAllTabs(groupId?: string): void {
  const state = get(tabs)
  const group = groupId
    ? state.groups.find((candidate) => candidate.id === groupId)
    : activeGroup(state)
  if (!group || !confirmCloseDirty(group.tabs)) return
  for (const tab of [...group.tabs]) closeTab(tab.id)
}

export function closeSavedTabs(groupId?: string): void {
  const state = get(tabs)
  const group = groupId
    ? state.groups.find((candidate) => candidate.id === groupId)
    : activeGroup(state)
  if (!group) return
  for (const tab of group.tabs.filter((candidate) => !(isEditorTab(candidate) && candidate.dirty))) {
    closeTab(tab.id)
  }
}

export function closeTabById(id: string): void {
  const tab = get(tabs).tabs.find((candidate) => candidate.id === id)
  if (!tab) return
  if (isEditorTab(tab) && tab.dirty && !confirm(`"${tab.name}" has unsaved changes. Close without saving?`)) return
  closeTab(id)
}

export function closeOtherTabs(keepId: string): void {
  const state = get(tabs)
  const group = groupForTab(state, keepId)
  if (!group) return
  const others = group.tabs.filter((tab) => tab.id !== keepId)
  if (!confirmCloseDirty(others)) return
  for (const tab of others) closeTab(tab.id)
}

export function closeTabsToRight(fromId: string): void {
  const state = get(tabs)
  const group = groupForTab(state, fromId)
  if (!group) return
  const idx = group.tabs.findIndex((tab) => tab.id === fromId)
  if (idx === -1) return
  const toClose = group.tabs.slice(idx + 1)
  if (!confirmCloseDirty(toClose)) return
  for (const tab of toClose) closeTab(tab.id)
}

export function cycleTab(dir: number): void {
  const group = activeGroup(get(tabs))
  if (group.tabs.length < 2) return
  const i = group.tabs.findIndex((tab) => tab.id === group.activeId)
  if (i === -1) return
  const next = (i + dir + group.tabs.length) % group.tabs.length
  activateTab(group.tabs[next].id, group.id)
}

export function closeTab(id: string): void {
  const state = get(tabs)
  const group = groupForTab(state, id)
  const closing = state.tabs.find((tab) => tab.id === id)
  if (!group || !closing) return
  const remainingAll = state.tabs.filter((tab) => tab.id !== id)
  if (isEditorTab(closing)) closeLspIfLast(closing.path, remainingAll)
  updateTabs((s) => ({
    ...s,
    groups: s.groups.map((candidate) => {
      if (candidate.id !== group.id) return candidate
      const idx = candidate.tabs.findIndex((tab) => tab.id === id)
      const remaining = candidate.tabs.filter((tab) => tab.id !== id)
      const next = remaining[idx] ?? remaining[idx - 1] ?? null
      return makeGroup({
        ...candidate,
        tabs: remaining,
        activeId: candidate.activeId === id ? next?.id ?? null : candidate.activeId,
      })
    }),
  }))
}

export function splitEditorGroup(orientation: EditorGroupOrientation = 'horizontal'): string {
  const state = get(tabs)
  const current = activeGroup(state)
  const active = current.tabs.find((tab) => tab.id === current.activeId)
  const clone = active ? cloneTab(active) : null
  const newGroup = makeGroup({
    tabs: clone ? [clone] : [],
    activeId: clone?.id ?? null,
  })
  const idx = state.groups.findIndex((group) => group.id === current.id)
  const groups = [...state.groups]
  groups.splice(idx + 1, 0, newGroup)
  updateTabs((s) => ({
    ...s,
    orientation,
    activeGroupId: newGroup.id,
    groups,
  }))
  if (clone && isEditorTab(clone) && clone.path) void lspOpenPath(clone.path, clone.state.doc.toString())
  return newGroup.id
}

export function createEditorGroup(orientation: EditorGroupOrientation = 'horizontal'): string {
  const state = get(tabs)
  const current = activeGroup(state)
  const newGroup = makeGroup()
  const idx = state.groups.findIndex((group) => group.id === current.id)
  const groups = [...state.groups]
  groups.splice(idx + 1, 0, newGroup)
  updateTabs((s) => ({
    ...s,
    orientation,
    activeGroupId: newGroup.id,
    groups,
  }))
  return newGroup.id
}

export function splitHorizontal(): void {
  splitEditorGroup('horizontal')
}

export function splitVertical(): void {
  splitEditorGroup('vertical')
}

export async function openFileToSide(path: string): Promise<void> {
  const groupId = createEditorGroup('horizontal')
  await openFile(path, { groupId, preview: false })
}

export function toggleLockActiveGroup(): void {
  const id = get(tabs).activeGroupId
  updateTabs((state) => ({
    ...state,
    groups: mapGroup(state, id, (group) => ({ ...group, isLocked: !group.isLocked })),
  }))
}

export function toggleMaximizeActiveGroup(): void {
  const id = get(tabs).activeGroupId
  const anyMaximized = get(tabs).groups.some((group) => group.isMaximized)
  updateTabs((state) => ({
    ...state,
    groups: state.groups.map((group) =>
      makeGroup({ ...group, isMaximized: anyMaximized ? false : group.id === id }),
    ),
  }))
}

export function setGroupSizes(sizes: Record<string, number>): void {
  updateTabs((state) => ({
    ...state,
    groups: state.groups.map((group) =>
      makeGroup({ ...group, size: Math.max(0.2, sizes[group.id] ?? group.size) }),
    ),
  }))
}

export function showOpenedEditors(): void {
  const lines = get(tabs).groups.flatMap((group, index) =>
    group.tabs.map((tab) => {
      const suffix = tab.id === group.activeId ? ' (active)' : ''
      return `Group ${index + 1}: ${tab.name}${suffix}`
    }),
  )
  alert(lines.length ? lines.join('\n') : 'No open editors.')
}

export function editorGroupsSnapshot(): PersistedEditorGroup[] {
  return get(tabs).groups.map((group) => {
    const active = group.tabs.find((tab) => tab.id === group.activeId)
    return {
      id: group.id,
      activeTabPath: active ? tabRestoreKey(active) : '',
      isLocked: group.isLocked,
      isMaximized: group.isMaximized,
      size: group.size,
      tabs: group.tabs
        .map((tab) => ({
          path: tabRestoreKey(tab),
          type: tab.kind,
          isDirty: isEditorTab(tab) ? tab.dirty : false,
          isPreview: !!tab.preview,
        }))
        .filter((tab) => !!tab.path),
    }
  })
}

export function resetEditorGroups(
  groups: Pick<EditorGroup, 'id' | 'isLocked' | 'isMaximized' | 'size'>[] = [],
  orientation: EditorGroupOrientation = 'horizontal',
  activeGroupId?: string,
): void {
  const nextGroups = groups.length
    ? groups.map((group) => makeGroup({ ...group, tabs: [], activeId: null }))
    : [makeGroup({ id: ROOT_GROUP_ID })]
  updateTabs(() => ({
    groups: nextGroups,
    activeGroupId: activeGroupId && nextGroups.some((group) => group.id === activeGroupId)
      ? activeGroupId
      : nextGroups[0].id,
    orientation,
  }))
}
