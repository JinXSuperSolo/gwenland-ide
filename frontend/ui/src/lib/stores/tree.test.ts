import { describe, expect, it } from 'vitest'
import { applyPatches, uniqueTreeRefreshPaths } from './tree'
import type { FlatRow, TreePatch } from '../tauri/commands'

/**
 * Tests for the pure flat-row patch reconciler (M19 Wave 2). This is the JS
 * mirror of the Rust-owned tree: it must splice Insert/Remove/Update deltas into
 * the row array in order, never reconstructing from scratch.
 */

function row(name: string, depth = 0, is_dir = false, is_expanded = false): FlatRow {
  return {
    id: `/ws/${name}`,
    name,
    path: `/ws/${name}`,
    depth,
    is_dir,
    is_expanded,
    has_children: is_dir,
  }
}

describe('applyPatches', () => {
  it('returns the same array reference when there are no patches', () => {
    const rows = [row('a'), row('b')]
    expect(applyPatches(rows, [])).toBe(rows)
  })

  it('inserts rows at the given index', () => {
    const rows = [row('dir', 0, true, true), row('after')]
    const patch: TreePatch = { kind: 'insert', index: 1, rows: [row('child', 1)] }
    const next = applyPatches(rows, [patch])
    expect(next.map((r) => r.name)).toEqual(['dir', 'child', 'after'])
  })

  it('removes a run of rows', () => {
    const rows = [row('dir', 0, true, true), row('c1', 1), row('c2', 1), row('after')]
    const patch: TreePatch = { kind: 'remove', index: 1, count: 2 }
    const next = applyPatches(rows, [patch])
    expect(next.map((r) => r.name)).toEqual(['dir', 'after'])
  })

  it('updates a single row in place', () => {
    const rows = [row('dir', 0, true, false)]
    const expanded = { ...rows[0], is_expanded: true }
    const patch: TreePatch = { kind: 'update', index: 0, row: expanded }
    const next = applyPatches(rows, [patch])
    expect(next[0].is_expanded).toBe(true)
  })

  it('applies an ordered batch (update then insert) like an expand', () => {
    const rows = [row('dir', 0, true, false), row('sibling')]
    const expanded = { ...rows[0], is_expanded: true }
    const patches: TreePatch[] = [
      { kind: 'update', index: 0, row: expanded },
      { kind: 'insert', index: 1, rows: [row('child', 1)] },
    ]
    const next = applyPatches(rows, patches)
    expect(next.map((r) => r.name)).toEqual(['dir', 'child', 'sibling'])
    expect(next[0].is_expanded).toBe(true)
  })

  it('applies a collapse batch (update then remove)', () => {
    const rows = [row('dir', 0, true, true), row('child', 1), row('sibling')]
    const collapsed = { ...rows[0], is_expanded: false }
    const patches: TreePatch[] = [
      { kind: 'update', index: 0, row: collapsed },
      { kind: 'remove', index: 1, count: 1 },
    ]
    const next = applyPatches(rows, patches)
    expect(next.map((r) => r.name)).toEqual(['dir', 'sibling'])
    expect(next[0].is_expanded).toBe(false)
  })

  it('does not mutate the input array', () => {
    const rows = [row('a')]
    applyPatches(rows, [{ kind: 'insert', index: 1, rows: [row('b')] }])
    expect(rows.map((r) => r.name)).toEqual(['a'])
  })
})

describe('uniqueTreeRefreshPaths', () => {
  it('keeps first-seen refresh paths and removes normalized duplicates', () => {
    expect(
      uniqueTreeRefreshPaths([
        'C:\\ws\\src\\',
        'c:/ws/src',
        'C:/ws/tests',
        'C:/ws/tests/',
        'C:/ws/README.md',
      ]),
    ).toEqual(['C:\\ws\\src\\', 'C:/ws/tests', 'C:/ws/README.md'])
  })
})
