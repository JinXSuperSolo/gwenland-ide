import { writable } from 'svelte/store'

/**
 * Inline tree input — drives the VS Code-style name-entry row that appears
 * directly in the file tree instead of a modal. One input at a time; opening a
 * second resolves the first as cancelled.
 */
export type TreeInputKind = 'file' | 'folder' | 'rename'

export interface TreeInputState {
  open: boolean
  kind: TreeInputKind
  /** Absolute path of the folder the new item will live inside (or the file being renamed). */
  targetDir: string
  /** Pre-filled value (used for rename). */
  initialValue: string
  /** Icon name to show in the input row. */
  icon: string
}

const closed: TreeInputState = {
  open: false,
  kind: 'file',
  targetDir: '',
  initialValue: '',
  icon: 'page',
}

export const treeInput = writable<TreeInputState>(closed)

let resolver: ((value: string | null) => void) | null = null

export function openTreeInput(opts: {
  kind: TreeInputKind
  targetDir: string
  initialValue?: string
  icon?: string
}): Promise<string | null> {
  if (resolver) {
    resolver(null)
    resolver = null
  }
  treeInput.set({
    open: true,
    kind: opts.kind,
    targetDir: opts.targetDir,
    initialValue: opts.initialValue ?? '',
    icon: opts.icon ?? (opts.kind === 'folder' ? 'folder' : 'page'),
  })
  return new Promise((resolve) => {
    resolver = resolve
  })
}

export function confirmTreeInput(value: string): void {
  const r = resolver
  resolver = null
  treeInput.set(closed)
  r?.(value.trim() || null)
}

export function cancelTreeInput(): void {
  const r = resolver
  resolver = null
  treeInput.set(closed)
  r?.(null)
}
