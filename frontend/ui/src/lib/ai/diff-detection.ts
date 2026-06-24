import { parseDiff, type DiffFile } from '../tauri/commands'

/**
 * AI response diff detection (Requirement 10.4-10.7).
 *
 * Detection is deliberately conservative: a cheap marker check gates the actual
 * (engine-backed) parse, and the parse must yield at least one file with at
 * least one hunk before we treat the response as a reviewable proposal. Parse
 * failures are non-destructive — the caller keeps the raw assistant text and may
 * surface a small notice.
 */

export type DiffDetection =
  | { kind: 'none' }
  | { kind: 'proposal'; files: DiffFile[]; fileCount: number; hunkCount: number }
  | { kind: 'failed'; message: string }

/** Cheap pre-check: does the text contain unified-diff markers? */
export function looksLikeDiff(text: string): boolean {
  let header = false
  let hunk = false
  for (const raw of text.split('\n')) {
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw
    if (line.startsWith('--- ') || line.startsWith('+++ ')) header = true
    else if (line.startsWith('@@')) hunk = true
    if (header && hunk) return true
  }
  return false
}

/**
 * Detect and parse a diff proposal from assistant text. Returns `none` when the
 * text has no diff markers (the common case — don't even call the parser),
 * `proposal` when a usable diff parses, or `failed` when it looked like a diff
 * but could not be parsed into a usable proposal.
 *
 * Files with zero hunks (e.g. a format example whose template header was skipped)
 * are dropped, so a placeholder diff never blocks review of the real change.
 */
export async function detectDiff(text: string): Promise<DiffDetection> {
  if (!looksLikeDiff(text)) return { kind: 'none' }
  try {
    const files = (await parseDiff(text)).filter((f) => f.hunks.length > 0)
    if (files.length === 0) {
      return { kind: 'failed', message: 'No complete diff hunks were found.' }
    }
    const hunkCount = files.reduce((n, f) => n + f.hunks.length, 0)
    return { kind: 'proposal', files, fileCount: files.length, hunkCount }
  } catch (e) {
    return { kind: 'failed', message: String(e) }
  }
}
