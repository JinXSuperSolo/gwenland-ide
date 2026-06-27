import { describe, expect, it } from 'vitest'
import type { FlatRow } from '../tauri/commands'
import { applyPatches } from './tree'
import {
  buildOptimisticCreatePatches,
  buildOptimisticMovePatches,
  buildOptimisticRemovePatches,
  joinPath,
} from './file-op-patches'

const ROOT = '/ws'

function row(
  name: string,
  depth = 0,
  is_dir = false,
  is_expanded = false,
  has_children = is_dir,
  parent = ROOT,
): FlatRow {
  const path = joinPath(parent, name)
  return {
    id: path,
    name,
    path,
    depth,
    is_dir,
    is_expanded,
    has_children,
  }
}

describe('optimistic file-op patches', () => {
  it('inserts created root children dirs first then case-insensitive name order', () => {
    const rows = [row('src', 0, true), row('b.txt'), row('z.txt')]
    const patches = buildOptimisticCreatePatches(rows, ROOT, '/ws/a.txt', false)
    const next = applyPatches(rows, patches)
    expect(next.map((r) => r.name)).toEqual(['src', 'a.txt', 'b.txt', 'z.txt'])
  })

  it('adds a created child inside an expanded folder', () => {
    const parent = row('src', 0, true, true)
    const rows = [parent, row('b.ts', 1, false, false, false, parent.path)]
    const patches = buildOptimisticCreatePatches(rows, ROOT, '/ws/src/a.ts', false)
    const next = applyPatches(rows, patches)
    expect(next.map((r) => `${r.depth}:${r.name}`)).toEqual(['0:src', '1:a.ts', '1:b.ts'])
  })

  it('marks a collapsed folder as having children without inserting hidden rows', () => {
    const parent = row('empty', 0, true, false, false)
    const patches = buildOptimisticCreatePatches([parent], ROOT, '/ws/empty/a.ts', false)
    expect(patches).toEqual([
      { kind: 'update', index: 0, row: { ...parent, has_children: true } },
    ])
  })

  it('removes a folder and its visible descendants', () => {
    const src = row('src', 0, true, true)
    const rows = [src, row('a.ts', 1, false, false, false, src.path), row('tail.txt')]
    const next = applyPatches(rows, buildOptimisticRemovePatches(rows, ROOT, src.path))
    expect(next.map((r) => r.name)).toEqual(['tail.txt'])
  })

  it('clears parent has_children when removing its last visible child', () => {
    const src = row('src', 0, true, true, true)
    const child = row('a.ts', 1, false, false, false, src.path)
    const next = applyPatches([src, child], buildOptimisticRemovePatches([src, child], ROOT, child.path))
    expect(next).toEqual([{ ...src, has_children: false }])
  })

  it('renames a visible subtree in place and preserves descendants', () => {
    const src = row('src', 0, true, true)
    const child = row('a.ts', 1, false, false, false, src.path)
    const rows = [src, child, row('tail.txt')]
    const next = applyPatches(rows, buildOptimisticMovePatches(rows, ROOT, '/ws/src', '/ws/app'))
    expect(next.map((r) => [r.name, r.path, r.depth])).toEqual([
      ['app', '/ws/app', 0],
      ['a.ts', '/ws/app/a.ts', 1],
      ['tail.txt', '/ws/tail.txt', 0],
    ])
  })

  it('moves a row into an expanded target folder with adjusted depth', () => {
    const src = row('src', 0, true, true)
    const oldChild = row('a.ts', 1, false, false, false, src.path)
    const target = row('target', 0, true, true)
    const rows = [src, oldChild, target]
    const next = applyPatches(rows, buildOptimisticMovePatches(rows, ROOT, oldChild.path, '/ws/target/a.ts'))
    expect(next.map((r) => `${r.depth}:${r.name}`)).toEqual(['0:src', '0:target', '1:a.ts'])
  })

  it('rejects moving a folder into itself', () => {
    const src = row('src', 0, true, true)
    const child = row('nested', 1, true, false, false, src.path)
    const rows = [src, child]
    expect(buildOptimisticMovePatches(rows, ROOT, src.path, '/ws/src/nested/src')).toEqual([])
  })
})
