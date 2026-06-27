import type { FlatRow, TreePatch } from '../tauri/commands'
import { applyPatches } from './tree'

export function pathSep(path: string): string {
  return path.includes('\\') ? '\\' : '/'
}

export function basename(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() || path
}

export function dirname(path: string): string {
  const idx = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'))
  return idx <= 0 ? path : path.slice(0, idx)
}

export function joinPath(parent: string, child: string): string {
  if (!parent) return child
  if (/^[a-zA-Z]:[\\/]/.test(child) || child.startsWith('/') || child.startsWith('\\')) {
    return child
  }
  const sep = pathSep(parent)
  return parent.endsWith(sep) ? parent + child : parent + sep + child
}

export function normalizePath(path: string): string {
  return path.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase()
}

export function samePath(a: string, b: string): boolean {
  return normalizePath(a) === normalizePath(b)
}

export function isDescendantPath(child: string, parent: string): boolean {
  const childN = normalizePath(child)
  const parentN = normalizePath(parent)
  return childN.startsWith(parentN + '/')
}

function compareNames(a: string, b: string): number {
  const al = a.toLowerCase()
  const bl = b.toLowerCase()
  if (al < bl) return -1
  if (al > bl) return 1
  if (a < b) return -1
  if (a > b) return 1
  return 0
}

function compareTreeOrder(a: Pick<FlatRow, 'name' | 'is_dir'>, b: Pick<FlatRow, 'name' | 'is_dir'>): number {
  if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1
  return compareNames(a.name, b.name)
}

function findRowIndex(rows: FlatRow[], path: string): number {
  const wanted = normalizePath(path)
  return rows.findIndex((row) => normalizePath(row.path) === wanted)
}

function runEnd(rows: FlatRow[], index: number): number {
  const depth = rows[index]?.depth
  if (depth === undefined) return index
  let end = index + 1
  while (end < rows.length && rows[end].depth > depth) end += 1
  return end
}

type ParentInfo =
  | { kind: 'root'; index: -1; depth: -1; expanded: true; path: string }
  | { kind: 'row'; index: number; depth: number; expanded: boolean; path: string; row: FlatRow }

function parentInfo(rows: FlatRow[], rootPath: string, parentPath: string): ParentInfo | null {
  if (samePath(parentPath, rootPath)) {
    return { kind: 'root', index: -1, depth: -1, expanded: true, path: rootPath }
  }
  const index = findRowIndex(rows, parentPath)
  if (index === -1) return null
  const row = rows[index]
  if (!row.is_dir) return null
  return {
    kind: 'row',
    index,
    depth: row.depth,
    expanded: row.is_expanded,
    path: row.path,
    row,
  }
}

function directChildRange(rows: FlatRow[], parent: ParentInfo): { start: number; end: number } {
  if (parent.kind === 'root') return { start: 0, end: rows.length }
  return { start: parent.index + 1, end: runEnd(rows, parent.index) }
}

function hasDirectChild(rows: FlatRow[], parent: ParentInfo): boolean {
  const { start, end } = directChildRange(rows, parent)
  for (let i = start; i < end; i += 1) {
    if (rows[i].depth === parent.depth + 1) return true
  }
  return false
}

function parentHasChildrenPatch(rows: FlatRow[], rootPath: string, parentPath: string, hasChildren: boolean): TreePatch[] {
  const parent = parentInfo(rows, rootPath, parentPath)
  if (!parent || parent.kind === 'root' || parent.row.has_children === hasChildren) return []
  return [{ kind: 'update', index: parent.index, row: { ...parent.row, has_children: hasChildren } }]
}

function insertionIndex(rows: FlatRow[], parent: ParentInfo, row: FlatRow): number {
  const { start, end } = directChildRange(rows, parent)
  let index = start
  while (index < end) {
    const candidate = rows[index]
    if (candidate.depth !== parent.depth + 1) {
      index += 1
      continue
    }
    if (compareTreeOrder(row, candidate) < 0) return index
    index = runEnd(rows, index)
  }
  return end
}

function flatRowFor(path: string, depth: number, isDir: boolean): FlatRow {
  return {
    id: path,
    name: basename(path),
    path,
    depth,
    is_dir: isDir,
    is_expanded: false,
    has_children: false,
  }
}

function insertRunPatches(rows: FlatRow[], rootPath: string, parentPath: string, run: FlatRow[]): TreePatch[] {
  if (run.length === 0) return []
  const parent = parentInfo(rows, rootPath, parentPath)
  if (!parent) return []

  const patches: TreePatch[] = []
  if (parent.kind === 'row' && !parent.row.has_children) {
    patches.push({ kind: 'update', index: parent.index, row: { ...parent.row, has_children: true } })
  }
  if (!parent.expanded) return patches

  const patchedRows = applyPatches(rows, patches)
  const patchedParent = parentInfo(patchedRows, rootPath, parentPath)
  if (!patchedParent || !patchedParent.expanded) return patches
  patches.push({ kind: 'insert', index: insertionIndex(patchedRows, patchedParent, run[0]), rows: run })
  return patches
}

export function buildOptimisticCreatePatches(
  rows: FlatRow[],
  rootPath: string,
  targetPath: string,
  isDir: boolean,
): TreePatch[] {
  const parentPath = dirname(targetPath)
  const parent = parentInfo(rows, rootPath, parentPath)
  if (!parent) return []
  const row = flatRowFor(targetPath, parent.depth + 1, isDir)
  return insertRunPatches(rows, rootPath, parentPath, [row])
}

export function buildOptimisticRemovePatches(
  rows: FlatRow[],
  rootPath: string,
  targetPath: string,
): TreePatch[] {
  const index = findRowIndex(rows, targetPath)
  if (index === -1) return []
  const count = runEnd(rows, index) - index
  const parentPath = dirname(rows[index].path)
  const patches: TreePatch[] = [{ kind: 'remove', index, count }]
  const afterRemove = applyPatches(rows, patches)
  const parent = parentInfo(afterRemove, rootPath, parentPath)
  if (parent && parent.kind === 'row' && !hasDirectChild(afterRemove, parent)) {
    patches.push(...parentHasChildrenPatch(afterRemove, rootPath, parentPath, false))
  }
  return patches
}

function rebaseRun(run: FlatRow[], oldPath: string, newPath: string, newTopDepth: number): FlatRow[] {
  const depthDelta = newTopDepth - run[0].depth
  return run.map((row, index) => {
    const suffix = index === 0 ? '' : row.path.slice(oldPath.length)
    const path = newPath + suffix
    return {
      ...row,
      id: path,
      name: index === 0 ? basename(newPath) : row.name,
      path,
      depth: row.depth + depthDelta,
    }
  })
}

export function buildOptimisticMovePatches(
  rows: FlatRow[],
  rootPath: string,
  oldPath: string,
  newPath: string,
): TreePatch[] {
  if (samePath(oldPath, newPath) || samePath(dirname(newPath), oldPath) || isDescendantPath(dirname(newPath), oldPath)) {
    return []
  }
  const index = findRowIndex(rows, oldPath)
  if (index === -1) return []
  const sourceRun = rows.slice(index, runEnd(rows, index))
  const removePatches = buildOptimisticRemovePatches(rows, rootPath, oldPath)
  const afterRemove = applyPatches(rows, removePatches)
  const newParent = parentInfo(afterRemove, rootPath, dirname(newPath))
  if (!newParent) return removePatches
  const rebased = rebaseRun(sourceRun, oldPath, newPath, newParent.depth + 1)
  return [...removePatches, ...insertRunPatches(afterRemove, rootPath, dirname(newPath), rebased)]
}
