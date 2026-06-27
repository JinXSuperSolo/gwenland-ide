import { get } from 'svelte/store'
import { describe, expect, it } from 'vitest'
import type { FlatRow } from '../tauri/commands'
import { treeRows } from './tree'
import {
  focusRow,
  resetTreeInteraction,
  selectRow,
  setActiveEditor,
  treeInteraction,
} from './tree-interaction'

function row(name: string): FlatRow {
  return {
    id: `/ws/${name}`,
    name,
    path: `/ws/${name}`,
    depth: 0,
    is_dir: false,
    is_expanded: false,
    has_children: false,
  }
}

describe('treeInteraction', () => {
  it('keeps selection and focus out of the tree row store', () => {
    resetTreeInteraction()
    const rows = [row('a.txt'), row('b.txt')]
    treeRows.set(rows)

    selectRow(rows[0].id)
    focusRow(rows[1].id)
    setActiveEditor(rows[0].path)

    expect(get(treeRows)).toBe(rows)
    expect(get(treeInteraction)).toEqual({
      selectedId: rows[0].id,
      focusedId: rows[1].id,
      activeEditorPath: rows[0].path,
    })
  })
})
