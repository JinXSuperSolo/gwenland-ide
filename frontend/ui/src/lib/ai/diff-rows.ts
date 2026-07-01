import type { DiffFile, DiffHunk, DiffLine } from '../tauri/commands'

/**
 * Pure diff → row-model transforms shared by every diff surface (GWEN-459).
 *
 * Takes the engine's structured `DiffFile`/`DiffHunk`/`DiffLine` (from
 * `parse_unified_diff`) and produces two row models:
 *
 *  - `unifiedRows(file)` — one flat list, each line classified add/del/ctx with
 *    its old and/or new line number (both tracked independently so numbers stay
 *    correct across insertions and deletions).
 *  - `splitRows(file)` — side-by-side pairs. Removed and added lines from the
 *    same change block are paired onto one visual row (old left / new right);
 *    unpaired insertions or deletions get an empty cell opposite. This pairing
 *    is the part that naive zipping gets wrong, so it's isolated + unit-tested.
 *
 * No I/O, no Svelte, no DOM — just data, so the pairing logic is testable on
 * its own.
 */

/** A cell on one side of a split row (or the single column of a unified row). */
export interface DiffCell {
  kind: 'add' | 'del' | 'ctx'
  text: string
  /** Line number in this cell's column, or null for a blank filler cell. */
  lineNo: number | null
}

/** One unified-view row: a single cell tracking both old and new line numbers. */
export interface UnifiedRow {
  kind: 'add' | 'del' | 'ctx' | 'hunk'
  text: string
  oldNo: number | null
  newNo: number | null
}

/** One split-view row: an old-side cell and a new-side cell (either may be blank). */
export interface SplitRow {
  kind: 'hunk' | 'pair'
  /** Hunk header text (only when kind === 'hunk'). */
  header?: string
  old: DiffCell | null
  new: DiffCell | null
}

/** Map the engine's serde-tagged `DiffLine.kind` onto the compact cell kind. */
function cellKind(line: DiffLine): 'add' | 'del' | 'ctx' {
  switch (line.kind) {
    case 'added':
      return 'add'
    case 'removed':
      return 'del'
    default:
      return 'ctx'
  }
}

/**
 * Unified rows for one file: hunk-header separators plus every line, walking
 * `old_start`/`new_start` and incrementing each counter only for the sides a
 * line actually occupies (context advances both; add only new; del only old).
 */
export function unifiedRows(file: DiffFile): UnifiedRow[] {
  const rows: UnifiedRow[] = []
  for (const hunk of file.hunks) {
    rows.push({ kind: 'hunk', text: hunk.header, oldNo: null, newNo: null })
    let oldNo = hunk.old_start
    let newNo = hunk.new_start
    for (const line of hunk.lines) {
      const kind = cellKind(line)
      if (kind === 'add') {
        rows.push({ kind, text: line.text, oldNo: null, newNo: newNo++ })
      } else if (kind === 'del') {
        rows.push({ kind, text: line.text, oldNo: oldNo++, newNo: null })
      } else {
        rows.push({ kind, text: line.text, oldNo: oldNo++, newNo: newNo++ })
      }
    }
  }
  return rows
}

/**
 * Split rows for one file. Within each hunk, a context line becomes one paired
 * row (same text both sides). A change block — a run of removed lines
 * immediately followed by a run of added lines — is paired index-by-index:
 * removed[i] on the left with added[i] on the right. Whichever run is longer
 * contributes rows with a blank cell on the opposite side. This keeps a
 * `del`-then-`add` block of unequal lengths aligned instead of drifting.
 */
export function splitRows(file: DiffFile): SplitRow[] {
  const rows: SplitRow[] = []
  for (const hunk of file.hunks) {
    rows.push({ kind: 'hunk', header: hunk.header, old: null, new: null })
    let oldNo = hunk.old_start
    let newNo = hunk.new_start

    // Pending change-block buffers, flushed when a context line (or hunk end)
    // breaks the run.
    let dels: DiffCell[] = []
    let adds: DiffCell[] = []

    const flush = () => {
      const n = Math.max(dels.length, adds.length)
      for (let i = 0; i < n; i++) {
        rows.push({ kind: 'pair', old: dels[i] ?? null, new: adds[i] ?? null })
      }
      dels = []
      adds = []
    }

    for (const line of hunk.lines) {
      const kind = cellKind(line)
      if (kind === 'del') {
        dels.push({ kind: 'del', text: line.text, lineNo: oldNo++ })
      } else if (kind === 'add') {
        adds.push({ kind: 'add', text: line.text, lineNo: newNo++ })
      } else {
        flush()
        rows.push({
          kind: 'pair',
          old: { kind: 'ctx', text: line.text, lineNo: oldNo++ },
          new: { kind: 'ctx', text: line.text, lineNo: newNo++ },
        })
      }
    }
    flush()
  }
  return rows
}

/** Total added / removed line counts across a file, for the header stat badge. */
export function fileStats(file: DiffFile): { added: number; removed: number } {
  let added = 0
  let removed = 0
  for (const hunk of file.hunks) {
    for (const line of hunk.lines) {
      if (line.kind === 'added') added++
      else if (line.kind === 'removed') removed++
    }
  }
  return { added, removed }
}

/** Display name for a file (new path preferred, falling back to old / label). */
export function fileDisplayPath(file: DiffFile): string {
  return file.new_path ?? file.old_path ?? '(new file)'
}

/** Basename + extension of a path, for the header filename + extension badge. */
export function fileNameParts(path: string): { name: string; ext: string } {
  const base = path.split(/[\\/]/).filter(Boolean).pop() ?? path
  const dot = base.lastIndexOf('.')
  if (dot <= 0) return { name: base, ext: '' }
  return { name: base, ext: base.slice(dot + 1) }
}

// Re-export the hunk type for callers that build rows per-hunk if needed.
export type { DiffHunk }
