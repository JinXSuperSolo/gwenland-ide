import { writable, derived, get } from 'svelte/store'
import type { DiffFile, DiffLine } from '../tauri/commands'
import { readFile, writeFile } from '../tauri/commands'
import { tabs, isEditorTab, saveActiveTab, setTabContent, openFile } from './tabs'
import { activeDoc, replaceActiveDocument } from '../editor/active-editor'

/**
 * Diff review session state (Milestone 8, Requirement 11). Holds the parsed
 * proposal as files → hunks with per-hunk status, plus the active file/hunk for
 * the editor overlay. This is runtime-only: accept/reject decisions are NEVER
 * persisted to conversation JSONL (Req 11.6).
 *
 * Applying accepted hunks (Req 13) is content-based: each hunk's old side
 * (context + removed lines) is located in the current file text and replaced
 * with its new side (context + added lines). This validates context, survives
 * unrelated edits, and leaves the file untouched when a hunk can't be matched.
 */

export type HunkStatus = 'pending' | 'accepted' | 'rejected' | 'failed'

export interface ReviewHunk {
  id: string
  fileId: string
  oldStart: number
  oldCount: number
  newStart: number
  newCount: number
  header: string
  lines: DiffLine[]
  status: HunkStatus
  error?: string
}

export interface ReviewFile {
  id: string
  oldPath: string | null
  newPath: string | null
  /** Absolute path used to open/save (resolved from the project root). */
  absPath: string | null
  hunks: ReviewHunk[]
}

export interface DiffReviewState {
  active: boolean
  proposalId: string | null
  files: ReviewFile[]
  activeFileId: string | null
  activeHunkId: string | null
}

const empty: DiffReviewState = {
  active: false,
  proposalId: null,
  files: [],
  activeFileId: null,
  activeHunkId: null,
}

export const diffReview = writable<DiffReviewState>(empty)

/** Summary counts for the action bar / panel (Req 11.4, 14.3). */
export const reviewSummary = derived(diffReview, (s) => {
  let hunks = 0
  let added = 0
  let removed = 0
  let accepted = 0
  let rejected = 0
  let failed = 0
  let pending = 0
  for (const f of s.files) {
    for (const h of f.hunks) {
      hunks++
      for (const l of h.lines) {
        if (l.kind === 'added') added++
        else if (l.kind === 'removed') removed++
      }
      if (h.status === 'accepted') accepted++
      else if (h.status === 'rejected') rejected++
      else if (h.status === 'failed') failed++
      else pending++
    }
  }
  return { files: s.files.length, hunks, added, removed, accepted, rejected, failed, pending }
})

// --- Path helpers ----------------------------------------------------------

function joinPath(root: string, rel: string): string {
  const sep = root.includes('\\') ? '\\' : '/'
  const base = root.replace(/[\\/]+$/, '')
  const cleaned = rel.replace(/^[\\/]+/, '').split(/[\\/]+/).join(sep)
  return `${base}${sep}${cleaned}`
}

/** Case-insensitive, separator-insensitive path comparison. */
export function sameFilePath(a: string | null, b: string | null): boolean {
  if (!a || !b) return false
  return a.replace(/\\/g, '/').toLowerCase() === b.replace(/\\/g, '/').toLowerCase()
}

// --- Lifecycle -------------------------------------------------------------

/** Begin a review session from a parsed proposal (Req 11.1). */
export function startReview(proposalId: string, files: DiffFile[], projectRoot: string | null): void {
  const reviewFiles: ReviewFile[] = files.map((f, fi) => {
    const fileId = `${proposalId}:f${fi}`
    const rel = f.new_path ?? f.old_path
    const absPath = rel && projectRoot ? joinPath(projectRoot, rel) : null
    return {
      id: fileId,
      oldPath: f.old_path,
      newPath: f.new_path,
      absPath,
      hunks: f.hunks.map((h, hi) => ({
        id: `${fileId}:h${hi}`,
        fileId,
        oldStart: h.old_start,
        oldCount: h.old_count,
        newStart: h.new_start,
        newCount: h.new_count,
        header: h.header,
        lines: h.lines,
        status: 'pending' as HunkStatus,
      })),
    }
  })
  const first = reviewFiles[0] ?? null
  diffReview.set({
    active: true,
    proposalId,
    files: reviewFiles,
    activeFileId: first?.id ?? null,
    activeHunkId: first?.hunks[0]?.id ?? null,
  })
  // Open the first file so its overlay is visible (best-effort).
  if (first?.absPath) void openFile(first.absPath)
}

/** Clear all review state + decorations (Req 11.5, 12.12). */
export function exitReview(): void {
  diffReview.set(empty)
}

export function setActiveFile(fileId: string): void {
  const s = get(diffReview)
  const file = s.files.find((f) => f.id === fileId)
  diffReview.update((st) => ({
    ...st,
    activeFileId: fileId,
    activeHunkId: file?.hunks[0]?.id ?? null,
  }))
  if (file?.absPath) void openFile(file.absPath)
}

// --- Per-hunk status -------------------------------------------------------

function setStatus(hunkId: string, status: HunkStatus, error?: string): void {
  diffReview.update((s) => ({
    ...s,
    files: s.files.map((f) => ({
      ...f,
      hunks: f.hunks.map((h) => (h.id === hunkId ? { ...h, status, error } : h)),
    })),
  }))
}

export function acceptHunk(hunkId: string): void {
  setStatus(hunkId, 'accepted')
  void finishIfResolved()
}

export function rejectHunk(hunkId: string): void {
  setStatus(hunkId, 'rejected')
  void finishIfResolved()
}

/** Accept All (Req 13.4): every still-pending hunk becomes accepted, then apply. */
export function acceptAll(): void {
  diffReview.update((s) => ({
    ...s,
    files: s.files.map((f) => ({
      ...f,
      hunks: f.hunks.map((h) => (h.status === 'pending' ? { ...h, status: 'accepted' } : h)),
    })),
  }))
  void finishIfResolved()
}

/** Reject All (Req 13.5): every still-pending hunk becomes rejected, then exit. */
export function rejectAll(): void {
  diffReview.update((s) => ({
    ...s,
    files: s.files.map((f) => ({
      ...f,
      hunks: f.hunks.map((h) => (h.status === 'pending' ? { ...h, status: 'rejected' } : h)),
    })),
  }))
  void finishIfResolved()
}

/** Cancel (Req 13.6): leave review without applying anything. */
export function cancelReview(): void {
  exitReview()
}

// --- Hunk navigation (keyboard) --------------------------------------------

/** Move the active hunk within the active file by `dir` (+1/-1). */
export function moveActiveHunk(dir: number): ReviewHunk | null {
  const s = get(diffReview)
  const file = s.files.find((f) => f.id === s.activeFileId)
  if (!file || file.hunks.length === 0) return null
  const idx = file.hunks.findIndex((h) => h.id === s.activeHunkId)
  const next = file.hunks[(Math.max(idx, 0) + dir + file.hunks.length) % file.hunks.length]
  diffReview.update((st) => ({ ...st, activeHunkId: next.id }))
  return next
}

export function activeHunk(): ReviewHunk | null {
  const s = get(diffReview)
  const file = s.files.find((f) => f.id === s.activeFileId)
  return file?.hunks.find((h) => h.id === s.activeHunkId) ?? null
}

// --- Applying accepted hunks (Req 13) --------------------------------------

/** When no hunks remain pending, apply accepted ones; keep open if any failed. */
async function finishIfResolved(): Promise<void> {
  const s = get(diffReview)
  if (!s.active) return
  const anyPending = s.files.some((f) => f.hunks.some((h) => h.status === 'pending'))
  if (anyPending) return
  await applyAccepted()
  const after = get(diffReview)
  const anyFailed = after.files.some((f) => f.hunks.some((h) => h.status === 'failed'))
  // Leave the session open to surface conflicts; exit cleanly otherwise.
  if (!anyFailed) exitReview()
}

/** Apply every accepted hunk to its file, saving only files that changed. */
async function applyAccepted(): Promise<void> {
  const files = get(diffReview).files
  for (const file of files) {
    const accepted = file.hunks.filter((h) => h.status === 'accepted')
    if (accepted.length === 0 || !file.absPath) continue
    await applyToFile(file, accepted)
  }
}

async function applyToFile(file: ReviewFile, accepted: ReviewHunk[]): Promise<void> {
  const tabsState = get(tabs)
  const tab = tabsState.tabs.find((t) => isEditorTab(t) && sameFilePath(t.path, file.absPath))
  const isActive = !!tab && tabsState.activeId === tab.id

  let curText: string
  if (isActive) curText = activeDoc() ?? (tab && isEditorTab(tab) ? tab.state.doc.toString() : '')
  else if (tab && isEditorTab(tab)) curText = tab.state.doc.toString()
  else {
    try {
      curText = await readFile(file.absPath!)
    } catch {
      for (const h of accepted) setStatus(h.id, 'failed', 'File could not be read')
      return
    }
  }

  const { text, failedIds } = applyHunksToText(curText, accepted)
  for (const id of failedIds) setStatus(id, 'failed', 'Could not match the original lines')
  if (failedIds.length === accepted.length) return // nothing applied → don't write

  if (isActive && replaceActiveDocument(text)) {
    await saveActiveTab()
  } else if (tab) {
    try {
      await writeFile(file.absPath!, text)
      setTabContent(tab.id, text)
    } catch {
      for (const h of accepted) if (!failedIds.includes(h.id)) setStatus(h.id, 'failed', 'Save failed')
    }
  } else {
    try {
      await writeFile(file.absPath!, text)
    } catch {
      for (const h of accepted) if (!failedIds.includes(h.id)) setStatus(h.id, 'failed', 'Save failed')
    }
  }
}

/**
 * Apply hunks to text by locating each hunk's old side and replacing it with its
 * new side. Pure + exported for unit-style reasoning; returns the new text and
 * the ids of hunks whose context could not be matched.
 */
export function applyHunksToText(
  text: string,
  hunks: ReviewHunk[]
): { text: string; failedIds: string[] } {
  const eol = text.includes('\r\n') ? '\r\n' : '\n'
  const lines = text.split(/\r?\n/)
  const failedIds: string[] = []
  // Apply top-to-bottom; re-locate against the mutated array each time.
  for (const h of [...hunks].sort((a, b) => a.oldStart - b.oldStart)) {
    const oldLines = h.lines.filter((l) => l.kind !== 'added').map((l) => l.text)
    const newLines = h.lines.filter((l) => l.kind !== 'removed').map((l) => l.text)
    const at = locateLines(lines, oldLines, h.oldStart - 1)
    if (at === -1) {
      failedIds.push(h.id)
      continue
    }
    lines.splice(at, oldLines.length, ...newLines)
  }
  return { text: lines.join(eol), failedIds }
}

/**
 * Find the 0-based index where `oldLines` occur in `lines`, preferring `nearIdx`
 * then scanning outward. Empty `oldLines` (pure addition) returns the insertion
 * index. Returns -1 when not found.
 */
export function locateLines(lines: string[], oldLines: string[], nearIdx: number): number {
  if (oldLines.length === 0) {
    return Math.min(Math.max(nearIdx + 1, 0), lines.length)
  }
  const len = oldLines.length
  const matches = (start: number): boolean => {
    if (start < 0 || start + len > lines.length) return false
    for (let i = 0; i < len; i++) if (lines[start + i] !== oldLines[i]) return false
    return true
  }
  if (matches(nearIdx)) return nearIdx
  for (let d = 1; d <= lines.length; d++) {
    if (matches(nearIdx - d)) return nearIdx - d
    if (matches(nearIdx + d)) return nearIdx + d
  }
  return -1
}
