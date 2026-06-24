import { writable, get } from 'svelte/store'

/**
 * Command/shortcut registry — the single source of truth for both the command
 * palette (GWEN-241) and global keyboard shortcuts (GWEN-243). Mirrors the
 * legacy shortcutRegistry: each command has an id, label, key combos, action.
 */
export interface Command {
  id: string
  label: string
  /** Normalized combos, e.g. ['Ctrl+Shift+P', 'Meta+Shift+P']. */
  keys: string[]
  action: () => void
}

const byId = new Map<string, Command>()
const byCombo = new Map<string, string>() // combo -> id

/** Reactive list for the palette UI; rebuilt on each register. */
export const commands = writable<Command[]>([])

export function registerCommand(
  id: string,
  label: string,
  keys: string[],
  action: () => void,
): void {
  byId.set(id, { id, label, keys, action })
  for (const k of keys) byCombo.set(k, id)
  commands.set([...byId.values()])
}

/**
 * Build a normalized combo string from a keydown event, e.g. "Ctrl+Shift+P".
 * Modifier order is fixed (Ctrl → Meta → Alt → Shift → key); single chars are
 * uppercased; named keys (Tab, Escape, `) pass through. Ported from the legacy
 * buildComboString.
 */
export function buildComboString(e: KeyboardEvent): string {
  const parts: string[] = []
  if (e.ctrlKey) parts.push('Ctrl')
  if (e.metaKey) parts.push('Meta')
  if (e.altKey) parts.push('Alt')
  if (e.shiftKey) parts.push('Shift')
  let key = e.key
  if (key === 'Control' || key === 'Meta' || key === 'Alt' || key === 'Shift') {
    return parts.join('+') // modifier-only
  }
  if (key.length === 1) key = key.toUpperCase()
  parts.push(key)
  return parts.join('+')
}

/**
 * Match a keydown to a registered command and run it. Returns true if handled
 * (caller should treat that as already preventDefault'd).
 */
export function dispatchShortcut(e: KeyboardEvent): boolean {
  const combo = buildComboString(e)
  const id = byCombo.get(combo)
  if (!id) return false
  const entry = byId.get(id)
  if (!entry) return false
  e.preventDefault()
  entry.action()
  return true
}

/** Filter commands by label/id substring (case-insensitive). */
export function filterCommands(query: string): Command[] {
  const q = query.trim().toLowerCase()
  const all = get(commands)
  if (!q) return all
  return all.filter(
    (c) => c.label.toLowerCase().includes(q) || c.id.toLowerCase().includes(q),
  )
}

/**
 * Display shortcut (the first registered combo) for a command id, or undefined.
 * The M9 context menu uses this so menu items show the same key hint as the
 * command palette / menu bar — one source of truth (Task 5.2).
 */
export function shortcutFor(id: string): string | undefined {
  return byId.get(id)?.keys[0]
}

/** Category label from a command id prefix (e.g. "file.save" → "File"). */
export function commandCategory(id: string): string {
  const p = (id.split('.')[0] || '').toLowerCase()
  const map: Record<string, string> = {
    file: 'File',
    tab: 'Tabs',
    panel: 'View',
    view: 'View',
    palette: 'View',
    search: 'Search',
    settings: 'Preferences',
    theme: 'Preferences',
  }
  return map[p] || (p ? p[0].toUpperCase() + p.slice(1) : '')
}
