import { describe, expect, it } from 'vitest'
import {
  classifyFile,
  LARGE_FILE_BYTES,
  LARGE_FILE_LINES,
  VERY_LARGE_FILE_BYTES,
} from './large-file'

/** Large File Mode classification thresholds (M19 Wave 3). */
describe('classifyFile', () => {
  it('treats a small file as normal', () => {
    expect(classifyFile({ size: 1000, line_count: 50 })).toEqual({ large: false, veryLarge: false })
  })

  it('flags large by size (> 500KB)', () => {
    const c = classifyFile({ size: LARGE_FILE_BYTES + 1, line_count: 10 })
    expect(c.large).toBe(true)
    expect(c.veryLarge).toBe(false)
  })

  it('flags large by line count (> 10k)', () => {
    const c = classifyFile({ size: 1000, line_count: LARGE_FILE_LINES + 1 })
    expect(c.large).toBe(true)
    expect(c.veryLarge).toBe(false)
  })

  it('does not flag exactly at the size threshold', () => {
    expect(classifyFile({ size: LARGE_FILE_BYTES, line_count: LARGE_FILE_LINES }).large).toBe(false)
  })

  it('flags very large (> 5MB) and implies large', () => {
    const c = classifyFile({ size: VERY_LARGE_FILE_BYTES + 1, line_count: 0 })
    expect(c.veryLarge).toBe(true)
    expect(c.large).toBe(true)
  })

  it('classifies a size-skipped count (u64::MAX) by size alone', () => {
    // Engine reports u64::MAX line count for files it didn't scan; size still
    // classifies it.
    const c = classifyFile({ size: VERY_LARGE_FILE_BYTES + 1, line_count: Number.MAX_SAFE_INTEGER })
    expect(c.large).toBe(true)
    expect(c.veryLarge).toBe(true)
  })
})
