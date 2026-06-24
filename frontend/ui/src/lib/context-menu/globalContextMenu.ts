import { openContextMenu } from './contextMenuStore'
import type { ContextMenuContext } from './contextTypes'

/**
 * Global context-menu plumbing (M9 follow-up). Two jobs:
 *
 *  1. A window-level fallback (`handleGlobalContextMenu`) so a right-click
 *     ANYWHERE shows the IDE menu and never the native OS/browser one — the
 *     context menu is the default everywhere. Surface-specific handlers
 *     `stopPropagation`, so this only runs for areas they don't cover.
 *  2. Smart routing (`openContextMenuSmart`): right-clicking a text field opens
 *     the shared `input` menu (Cut/Copy/Paste/Select All) instead of the pane
 *     menu, mirroring VS Code. The field + the selection range at click time are
 *     captured so the clipboard ops act on exactly what was right-clicked.
 */

let inputTarget: HTMLInputElement | HTMLTextAreaElement | null = null
let inputRange: { start: number; end: number } | null = null

function isTextField(el: Element | null): el is HTMLInputElement | HTMLTextAreaElement {
  return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement
}

/** Open the `input` menu for a right-clicked text field, else the `fallback`. */
export function openContextMenuSmart(e: MouseEvent, fallback: ContextMenuContext): void {
  const field = (e.target as HTMLElement | null)?.closest('input, textarea') ?? null
  if (isTextField(field)) {
    inputTarget = field
    inputRange = {
      start: field.selectionStart ?? 0,
      end: field.selectionEnd ?? field.value.length,
    }
    openContextMenu(e, { scope: 'input' })
  } else {
    inputTarget = null
    inputRange = null
    openContextMenu(e, fallback)
  }
}

/** Window fallback: never show the native menu; show the IDE menu everywhere. */
export function handleGlobalContextMenu(e: MouseEvent): void {
  openContextMenuSmart(e, {
    scope: 'global',
    selectionText: window.getSelection()?.toString() || undefined,
  })
}

// --- Input clipboard helpers (operate on the captured field + range) --------

export function inputHasSelection(): boolean {
  return !!inputRange && inputRange.start !== inputRange.end
}

export function inputHasValue(): boolean {
  return !!inputTarget && inputTarget.value.length > 0
}

export async function inputCopy(): Promise<void> {
  if (!inputTarget || !inputRange) return
  const text = inputTarget.value.substring(inputRange.start, inputRange.end)
  if (text) await navigator.clipboard.writeText(text).catch(() => {})
}

export async function inputCut(): Promise<void> {
  const el = inputTarget
  const range = inputRange
  if (!el || !range || range.start === range.end) return
  const text = el.value.substring(range.start, range.end)
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    return // don't delete if the copy failed
  }
  el.focus()
  el.setRangeText('', range.start, range.end, 'end')
  el.dispatchEvent(new Event('input', { bubbles: true })) // sync framework bindings
}

export async function inputPaste(): Promise<void> {
  const el = inputTarget
  if (!el) return
  let text = ''
  try {
    text = await navigator.clipboard.readText()
  } catch {
    return
  }
  if (!text) return
  el.focus()
  const start = inputRange?.start ?? el.value.length
  const end = inputRange?.end ?? el.value.length
  el.setRangeText(text, start, end, 'end')
  el.dispatchEvent(new Event('input', { bubbles: true }))
}

export function inputSelectAll(): void {
  inputTarget?.focus()
  inputTarget?.select()
}
