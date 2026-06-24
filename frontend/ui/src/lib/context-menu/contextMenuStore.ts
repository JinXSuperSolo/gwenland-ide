import { writable } from 'svelte/store'
import type { ContextMenuContext } from './contextTypes'

/**
 * Open/close state for the single context-menu shell. There is exactly one menu
 * on screen at a time; opening a new one replaces the old. `x`/`y` are the
 * viewport-relative pointer coordinates the portal positions (and clamps) at.
 */
export interface ContextMenuState {
  open: boolean
  x: number
  y: number
  context: ContextMenuContext | null
}

const initial: ContextMenuState = { open: false, x: 0, y: 0, context: null }

export const contextMenuStore = writable<ContextMenuState>(initial)

/**
 * The one API every surface calls (Requirement 4.1). Prevents the native browser
 * menu, then opens the registry-driven menu at the pointer with the given
 * context. For keyboard invocation (Shift+F10 / the context-menu key), pass a
 * synthetic position via a `MouseEvent`-like object.
 */
export function openContextMenu(event: MouseEvent, ctx: ContextMenuContext): void {
  event.preventDefault()
  event.stopPropagation()
  contextMenuStore.set({ open: true, x: event.clientX, y: event.clientY, context: ctx })
}

/** Close the menu (no-op if already closed, so it's cheap to call defensively). */
export function closeContextMenu(): void {
  contextMenuStore.update((s) => (s.open ? { ...s, open: false } : s))
}
