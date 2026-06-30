import type { FlatRow } from '../tauri/commands'

/**
 * Pure keyboard-navigation logic for the flat virtual file tree (M-keynav).
 *
 * The tree is a flat `FlatRow[]` where collapsed folders simply have no child
 * rows present — so arrow navigation over the array already respects the
 * expand/collapse state for free. These helpers compute *intent* from a key +
 * the current rows + the current selection; the store/component layer turns that
 * intent into the async expand/collapse/open side effects. Keeping the decision
 * pure makes every arrow-key behavior unit-testable without a DOM.
 */

/** What the component should do in response to an arrow/enter key. */
export type TreeNavAction =
  | { kind: 'none' }
  /** Move the selection (and focus) to `id`, scrolling it into view. */
  | { kind: 'select'; id: string }
  /** Expand the folder at `path`; selection stays on it. */
  | { kind: 'expand'; path: string }
  /** Collapse the folder at `path`; selection stays on it. */
  | { kind: 'collapse'; path: string }
  /** Activate the row at `id` (open file, or toggle folder). */
  | { kind: 'activate'; id: string }

/** Index of the currently selected row, or -1 if none/stale. */
export function selectedIndex(rows: FlatRow[], selectedId: string | null): number {
  if (!selectedId) return -1
  return rows.findIndex((row) => row.id === selectedId)
}

/**
 * The nearest preceding row whose depth is exactly one less than `rows[index]`'s
 * — i.e. the parent folder in the flattened tree. Returns -1 at the root level.
 */
export function parentIndex(rows: FlatRow[], index: number): number {
  if (index < 0 || index >= rows.length) return -1
  const depth = rows[index].depth
  for (let i = index - 1; i >= 0; i--) {
    if (rows[i].depth < depth) return i
  }
  return -1
}

/**
 * Decide the navigation action for `key`, given the current rows and selection.
 * Mirrors VS Code's Explorer arrow behavior:
 *   - Down/Up: move to the next/previous visible row.
 *   - Right on a collapsed folder: expand it; on an expanded folder: select its
 *     first child; on a file: no-op.
 *   - Left on an expanded folder: collapse it; otherwise (collapsed folder or
 *     file): select the parent folder.
 *   - Enter: activate (open file / toggle folder).
 * With no current selection, the first arrow keypress selects the first row.
 */
export function navigate(
  rows: FlatRow[],
  selectedId: string | null,
  key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight' | 'Enter',
): TreeNavAction {
  if (rows.length === 0) return { kind: 'none' }

  const index = selectedIndex(rows, selectedId)

  // No (or stale) selection: the first directional key lands on the first row.
  if (index === -1) {
    if (key === 'ArrowDown' || key === 'ArrowUp' || key === 'ArrowRight' || key === 'ArrowLeft') {
      return { kind: 'select', id: rows[0].id }
    }
    return { kind: 'none' }
  }

  const row = rows[index]

  switch (key) {
    case 'ArrowDown': {
      const next = index + 1
      return next < rows.length ? { kind: 'select', id: rows[next].id } : { kind: 'none' }
    }
    case 'ArrowUp': {
      const prev = index - 1
      return prev >= 0 ? { kind: 'select', id: rows[prev].id } : { kind: 'none' }
    }
    case 'ArrowRight': {
      if (row.is_dir && !row.is_expanded) return { kind: 'expand', path: row.path }
      if (row.is_dir && row.is_expanded) {
        const child = index + 1
        // Only step in if the next row is actually this folder's child.
        if (child < rows.length && rows[child].depth > row.depth) {
          return { kind: 'select', id: rows[child].id }
        }
      }
      return { kind: 'none' }
    }
    case 'ArrowLeft': {
      if (row.is_dir && row.is_expanded) return { kind: 'collapse', path: row.path }
      const parent = parentIndex(rows, index)
      return parent >= 0 ? { kind: 'select', id: rows[parent].id } : { kind: 'none' }
    }
    case 'Enter':
      return { kind: 'activate', id: row.id }
  }
}
