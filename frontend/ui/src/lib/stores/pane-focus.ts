import { writable } from 'svelte/store'

/**
 * Pane focus tracking + Tab pane-cycling (M-keynav §3).
 *
 * Tab / Shift+Tab move keyboard focus between the IDE's major panes in a fixed
 * order — Sidebar → Editor → Terminal → (wrap). The Terminal is skipped when it
 * isn't available (collapsed, or no workspace open). The focused pane gets a
 * visible accent ring (driven by the `focusedPane` store, see App.svelte).
 *
 * The cycle order and "which panes are available" decision are kept as pure
 * functions so they can be unit-tested without a DOM, and so the global keydown
 * handler stays a thin shell.
 */

export type Pane = 'sidebar' | 'editor' | 'terminal'

/** Canonical forward cycle order. Shift+Tab walks it in reverse. */
export const PANE_ORDER: Pane[] = ['sidebar', 'editor', 'terminal']

/** The pane that currently owns keyboard focus (null until first Tab). */
export const focusedPane = writable<Pane | null>(null)

export interface PaneAvailability {
  /** The left sidebar (Files / Agent) — only present when a folder is open. */
  sidebar: boolean
  /** The editor pane — always present. */
  editor: boolean
  /** The bottom terminal — present only when expanded with a workspace. */
  terminal: boolean
}

/** The available panes in cycle order, given current availability. Pure. */
export function availablePanes(avail: PaneAvailability): Pane[] {
  return PANE_ORDER.filter((pane) => avail[pane])
}

/**
 * Compute the next pane to focus when Tab (or Shift+Tab) is pressed.
 *   - `current` is the pane that has focus now (or null if none yet).
 *   - `dir` is +1 for Tab (forward) or -1 for Shift+Tab (reverse).
 * Returns null only when no pane is available at all (degenerate). When the
 * current pane is unavailable/unknown, focus lands on the first (or last, in
 * reverse) available pane. Wraps at the ends. Pure.
 */
export function nextPane(
  current: Pane | null,
  dir: 1 | -1,
  avail: PaneAvailability,
): Pane | null {
  const panes = availablePanes(avail)
  if (panes.length === 0) return null
  const idx = current ? panes.indexOf(current) : -1
  if (idx === -1) return dir === 1 ? panes[0] : panes[panes.length - 1]
  const next = (idx + dir + panes.length) % panes.length
  return panes[next]
}

/**
 * Whether a Tab press should be treated as pane-cycling rather than left to the
 * focused control. Pane-cycling is suppressed while focus is inside an editable
 * surface (so in-editor Tab keeps indenting, and form inputs keep their Tab):
 *   - the CodeMirror editor (`.cm-content`, contenteditable)
 *   - any <input> / <textarea> / [contenteditable] element
 * Pure given the active element.
 */
export function shouldCyclePanes(active: Element | null): boolean {
  if (!active) return true
  const tag = active.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA') return false
  // CodeMirror's editable surface is a contenteditable div.
  if ((active as HTMLElement).isContentEditable) return false
  if (active.closest('.cm-editor')) return false
  return true
}

/** Update the focused-pane store (used by the keydown handler + tests). */
export function setFocusedPane(pane: Pane | null): void {
  focusedPane.set(pane)
}
