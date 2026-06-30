import { describe, expect, it } from 'vitest'
import type { FlatRow } from '../tauri/commands'
import { navigate, parentIndex, selectedIndex } from './tree-navigation'

/**
 * Behavioral tests for file-tree keyboard navigation (M-keynav §1). The flat
 * row array is the real shape the virtual tree renders; collapsed folders simply
 * have no child rows present, so navigation over the array already respects the
 * expand/collapse state. Each case simulates a key against a concrete tree and
 * asserts the resulting navigation intent.
 */

function dir(name: string, depth: number, expanded: boolean): FlatRow {
  return {
    id: `id:${name}`,
    name,
    path: `/ws/${name}`,
    depth,
    is_dir: true,
    is_expanded: expanded,
    has_children: true,
  }
}

function file(name: string, depth: number): FlatRow {
  return {
    id: `id:${name}`,
    name,
    path: `/ws/${name}`,
    depth,
    is_dir: false,
    is_expanded: false,
    has_children: false,
  }
}

// A small tree:
//   src/            (dir, expanded)      idx 0
//     a.ts          (file, depth 1)      idx 1
//     b.ts          (file, depth 1)      idx 2
//   docs/           (dir, collapsed)     idx 3
//   README.md       (file, depth 0)      idx 4
function tree(): FlatRow[] {
  return [
    dir('src', 0, true),
    file('a.ts', 1),
    file('b.ts', 1),
    dir('docs', 0, false),
    file('README.md', 0),
  ]
}

describe('tree-navigation helpers', () => {
  it('finds the parent folder index by depth', () => {
    const rows = tree()
    expect(parentIndex(rows, 1)).toBe(0) // a.ts -> src
    expect(parentIndex(rows, 2)).toBe(0) // b.ts -> src
    expect(parentIndex(rows, 4)).toBe(-1) // README.md is at root
  })

  it('reports the selected index, -1 when missing', () => {
    const rows = tree()
    expect(selectedIndex(rows, 'id:b.ts')).toBe(2)
    expect(selectedIndex(rows, null)).toBe(-1)
    expect(selectedIndex(rows, 'id:gone')).toBe(-1)
  })
})

describe('navigate', () => {
  it('Down/Up move between visible rows', () => {
    const rows = tree()
    expect(navigate(rows, 'id:src', 'ArrowDown')).toEqual({ kind: 'select', id: 'id:a.ts' })
    expect(navigate(rows, 'id:a.ts', 'ArrowUp')).toEqual({ kind: 'select', id: 'id:src' })
  })

  it('Down stops at the last row, Up stops at the first', () => {
    const rows = tree()
    expect(navigate(rows, 'id:README.md', 'ArrowDown')).toEqual({ kind: 'none' })
    expect(navigate(rows, 'id:src', 'ArrowUp')).toEqual({ kind: 'none' })
  })

  it('first arrow with no selection lands on the first row', () => {
    const rows = tree()
    expect(navigate(rows, null, 'ArrowDown')).toEqual({ kind: 'select', id: 'id:src' })
    expect(navigate(rows, null, 'ArrowUp')).toEqual({ kind: 'select', id: 'id:src' })
  })

  it('Right expands a collapsed folder', () => {
    const rows = tree()
    expect(navigate(rows, 'id:docs', 'ArrowRight')).toEqual({ kind: 'expand', path: '/ws/docs' })
  })

  it('Right on an expanded folder steps into its first child', () => {
    const rows = tree()
    expect(navigate(rows, 'id:src', 'ArrowRight')).toEqual({ kind: 'select', id: 'id:a.ts' })
  })

  it('Right on a file is a no-op', () => {
    const rows = tree()
    expect(navigate(rows, 'id:README.md', 'ArrowRight')).toEqual({ kind: 'none' })
  })

  it('Left collapses an expanded folder', () => {
    const rows = tree()
    expect(navigate(rows, 'id:src', 'ArrowLeft')).toEqual({ kind: 'collapse', path: '/ws/src' })
  })

  it('Left on a child selects its parent folder', () => {
    const rows = tree()
    expect(navigate(rows, 'id:a.ts', 'ArrowLeft')).toEqual({ kind: 'select', id: 'id:src' })
  })

  it('Left on a collapsed folder at root is a no-op', () => {
    const rows = tree()
    expect(navigate(rows, 'id:docs', 'ArrowLeft')).toEqual({ kind: 'none' })
  })

  it('Enter activates the selected row (file or folder)', () => {
    const rows = tree()
    expect(navigate(rows, 'id:README.md', 'Enter')).toEqual({ kind: 'activate', id: 'id:README.md' })
    expect(navigate(rows, 'id:docs', 'Enter')).toEqual({ kind: 'activate', id: 'id:docs' })
  })

  it('does not crash on an empty tree', () => {
    expect(navigate([], null, 'ArrowDown')).toEqual({ kind: 'none' })
    expect(navigate([], 'id:gone', 'Enter')).toEqual({ kind: 'none' })
  })

  it('does not navigate into a collapsed folder (children are absent)', () => {
    // docs/ is collapsed, so the row after it is README.md at root — Down from
    // docs must land on README.md, never on a hidden child.
    const rows = tree()
    expect(navigate(rows, 'id:docs', 'ArrowDown')).toEqual({ kind: 'select', id: 'id:README.md' })
  })
})
