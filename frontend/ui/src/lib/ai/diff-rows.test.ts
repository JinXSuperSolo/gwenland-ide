import { describe, expect, it } from 'vitest'
import { unifiedRows, splitRows, fileStats, fileNameParts, fileDisplayPath } from './diff-rows'
import type { DiffFile, DiffLine } from '../tauri/commands'

const ctx = (text: string): DiffLine => ({ kind: 'context', text })
const add = (text: string): DiffLine => ({ kind: 'added', text })
const del = (text: string): DiffLine => ({ kind: 'removed', text })

function file(hunks: DiffFile['hunks']): DiffFile {
  return { old_path: 'src/a.ts', new_path: 'src/a.ts', hunks }
}

describe('unifiedRows', () => {
  it('tracks old and new line numbers independently across an edit', () => {
    const f = file([
      {
        old_start: 10,
        old_count: 3,
        new_start: 10,
        new_count: 3,
        header: '@@ -10,3 +10,3 @@',
        lines: [ctx('a'), del('b'), add('B'), ctx('c')],
      },
    ])
    const rows = unifiedRows(f)
    expect(rows.map((r) => [r.kind, r.oldNo, r.newNo])).toEqual([
      ['hunk', null, null],
      ['ctx', 10, 10],
      ['del', 11, null],
      ['add', null, 11],
      ['ctx', 12, 12],
    ])
  })

  it('advances only the new counter for a pure insertion', () => {
    const f = file([
      {
        old_start: 5,
        old_count: 1,
        new_start: 5,
        new_count: 3,
        header: '@@ -5 +5,3 @@',
        lines: [ctx('keep'), add('new1'), add('new2')],
      },
    ])
    const rows = unifiedRows(f).filter((r) => r.kind !== 'hunk')
    expect(rows).toEqual([
      { kind: 'ctx', text: 'keep', oldNo: 5, newNo: 5 },
      { kind: 'add', text: 'new1', oldNo: null, newNo: 6 },
      { kind: 'add', text: 'new2', oldNo: null, newNo: 7 },
    ])
  })
})

describe('splitRows', () => {
  it('pairs a context line onto both sides with matching numbers', () => {
    const f = file([
      {
        old_start: 1,
        old_count: 1,
        new_start: 1,
        new_count: 1,
        header: '@@ -1 +1 @@',
        lines: [ctx('same')],
      },
    ])
    const rows = splitRows(f)
    expect(rows[0].kind).toBe('hunk')
    expect(rows[1]).toEqual({
      kind: 'pair',
      old: { kind: 'ctx', text: 'same', lineNo: 1 },
      new: { kind: 'ctx', text: 'same', lineNo: 1 },
    })
  })

  it('pairs equal-length del/add blocks index-by-index', () => {
    const f = file([
      {
        old_start: 1,
        old_count: 2,
        new_start: 1,
        new_count: 2,
        header: '@@ -1,2 +1,2 @@',
        lines: [del('x1'), del('x2'), add('y1'), add('y2')],
      },
    ])
    const pairs = splitRows(f).filter((r) => r.kind === 'pair')
    expect(pairs).toEqual([
      { kind: 'pair', old: { kind: 'del', text: 'x1', lineNo: 1 }, new: { kind: 'add', text: 'y1', lineNo: 1 } },
      { kind: 'pair', old: { kind: 'del', text: 'x2', lineNo: 2 }, new: { kind: 'add', text: 'y2', lineNo: 2 } },
    ])
  })

  it('gives the shorter side blank cells when a change block is uneven (2 del, 4 add)', () => {
    const f = file([
      {
        old_start: 1,
        old_count: 2,
        new_start: 1,
        new_count: 4,
        header: '@@ -1,2 +1,4 @@',
        lines: [del('d1'), del('d2'), add('a1'), add('a2'), add('a3'), add('a4')],
      },
    ])
    const pairs = splitRows(f).filter((r) => r.kind === 'pair')
    expect(pairs).toEqual([
      { kind: 'pair', old: { kind: 'del', text: 'd1', lineNo: 1 }, new: { kind: 'add', text: 'a1', lineNo: 1 } },
      { kind: 'pair', old: { kind: 'del', text: 'd2', lineNo: 2 }, new: { kind: 'add', text: 'a2', lineNo: 2 } },
      { kind: 'pair', old: null, new: { kind: 'add', text: 'a3', lineNo: 3 } },
      { kind: 'pair', old: null, new: { kind: 'add', text: 'a4', lineNo: 4 } },
    ])
  })

  it('gives the new side blank cells when there are more deletions (3 del, 1 add)', () => {
    const f = file([
      {
        old_start: 7,
        old_count: 3,
        new_start: 7,
        new_count: 1,
        header: '@@ -7,3 +7,1 @@',
        lines: [del('r1'), del('r2'), del('r3'), add('n1')],
      },
    ])
    const pairs = splitRows(f).filter((r) => r.kind === 'pair')
    expect(pairs).toEqual([
      { kind: 'pair', old: { kind: 'del', text: 'r1', lineNo: 7 }, new: { kind: 'add', text: 'n1', lineNo: 7 } },
      { kind: 'pair', old: { kind: 'del', text: 'r2', lineNo: 8 }, new: null },
      { kind: 'pair', old: { kind: 'del', text: 'r3', lineNo: 9 }, new: null },
    ])
  })

  it('keeps line numbers correct across context that separates two change blocks', () => {
    // ctx, then a del/add swap, then ctx, then a pure insertion.
    const f = file([
      {
        old_start: 1,
        old_count: 3,
        new_start: 1,
        new_count: 4,
        header: '@@ -1,3 +1,4 @@',
        lines: [ctx('c1'), del('old'), add('new'), ctx('c2'), add('ins')],
      },
    ])
    const pairs = splitRows(f).filter((r) => r.kind === 'pair')
    expect(pairs).toEqual([
      { kind: 'pair', old: { kind: 'ctx', text: 'c1', lineNo: 1 }, new: { kind: 'ctx', text: 'c1', lineNo: 1 } },
      { kind: 'pair', old: { kind: 'del', text: 'old', lineNo: 2 }, new: { kind: 'add', text: 'new', lineNo: 2 } },
      { kind: 'pair', old: { kind: 'ctx', text: 'c2', lineNo: 3 }, new: { kind: 'ctx', text: 'c2', lineNo: 3 } },
      { kind: 'pair', old: null, new: { kind: 'add', text: 'ins', lineNo: 4 } },
    ])
  })
})

describe('fileStats / helpers', () => {
  it('counts added and removed lines across hunks', () => {
    const f = file([
      {
        old_start: 1, old_count: 1, new_start: 1, new_count: 2,
        header: '@@', lines: [del('a'), add('b'), add('c'), ctx('d')],
      },
    ])
    expect(fileStats(f)).toEqual({ added: 2, removed: 1 })
  })

  it('splits a filename into name + extension', () => {
    expect(fileNameParts('src/lib/foo.svelte')).toEqual({ name: 'foo.svelte', ext: 'svelte' })
    expect(fileNameParts('Makefile')).toEqual({ name: 'Makefile', ext: '' })
    expect(fileNameParts('.gitignore')).toEqual({ name: '.gitignore', ext: '' })
  })

  it('prefers the new path for display', () => {
    expect(fileDisplayPath({ old_path: 'old.ts', new_path: 'new.ts', hunks: [] })).toBe('new.ts')
    expect(fileDisplayPath({ old_path: 'only-old.ts', new_path: null, hunks: [] })).toBe('only-old.ts')
    expect(fileDisplayPath({ old_path: null, new_path: null, hunks: [] })).toBe('(new file)')
  })
})
