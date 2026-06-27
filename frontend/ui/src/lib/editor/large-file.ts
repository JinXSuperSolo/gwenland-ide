import type { FileMeta } from '../tauri/commands'

/**
 * Large File Mode classification (M19 Wave 3, GWEN-377).
 *
 * Large files freeze the editor when every feature (syntax highlight, LSP,
 * folding, minimap…) runs over them. We classify a file from its size + line
 * count and disable the expensive features. Very large files additionally open
 * read-only as plain text.
 *
 * Thresholds (from the M19 plan):
 *   - large:      size > 500 KB  OR  line_count > 10,000
 *   - very large: size > 5 MB
 */
export const LARGE_FILE_BYTES = 500_000
export const LARGE_FILE_LINES = 10_000
export const VERY_LARGE_FILE_BYTES = 5_000_000

export interface LargeFileClass {
  /** Reduced-feature mode: no syntax/LSP/minimap/sticky scroll. */
  large: boolean
  /** Read-only plain text (implies `large`). */
  veryLarge: boolean
}

const NOT_LARGE: LargeFileClass = { large: false, veryLarge: false }

/** Classify a file from its metadata. A skipped line count (u64::MAX) still
 *  trips the size thresholds, so very large files classify correctly. */
export function classifyFile(meta: FileMeta): LargeFileClass {
  const veryLarge = meta.size > VERY_LARGE_FILE_BYTES
  const large = veryLarge || meta.size > LARGE_FILE_BYTES || meta.line_count > LARGE_FILE_LINES
  return { large, veryLarge }
}

export { NOT_LARGE }
