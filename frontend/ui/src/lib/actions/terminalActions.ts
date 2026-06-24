/**
 * Terminal context actions (Requirement 7.1). Copy/Paste/Clear/Select All act on
 * the specific xterm instance the menu was opened over (looked up by session key
 * via the terminal registry); New/Split/Kill drive the sessions store. No new
 * menu component — all through the shared registry (Requirement 7.6).
 *
 * Split Terminal opens a new session tab (the panel has no split panes yet — a
 * graceful equivalent of "another terminal").
 */
import { get } from 'svelte/store'
import { registry } from '../context-menu/actionRegistry'
import type { ContextAction, ContextMenuContext } from '../context-menu/contextTypes'
import { getTerminalHandle } from '../terminal/terminal-registry'
import { createSession, removeSession, terminalSessions } from '../stores/terminal-sessions'
import { collapsePanel } from '../stores/panels'
import { terminalKill } from '../tauri/commands'

function handleFor(ctx: ContextMenuContext) {
  return getTerminalHandle(ctx.terminalId)
}

/** Kill a session: drop it, kill its PTY, and collapse the panel if it emptied. */
function killTerminal(key: string): void {
  const ptyId = removeSession(key)
  if (ptyId) void terminalKill(ptyId).catch(() => {})
  if (get(terminalSessions).sessions.length === 0) collapsePanel('terminal')
}

const terminalActions: ContextAction[] = [
  // ── clipboard ─────────────────────────────────────────────────────────────
  {
    id: 'terminal.copy',
    label: 'Copy',
    icon: 'copy',
    group: 'clipboard',
    order: 10,
    shortcut: 'Ctrl+Shift+C',
    when: (ctx) => ctx.scope === 'terminal',
    enabled: (ctx) => !!ctx.terminalSelection && !!handleFor(ctx),
    run: (ctx) => void handleFor(ctx)?.copySelection(),
  },
  {
    id: 'terminal.paste',
    label: 'Paste',
    icon: 'clipboard-check',
    group: 'clipboard',
    order: 20,
    shortcut: 'Ctrl+Shift+V',
    when: (ctx) => ctx.scope === 'terminal',
    enabled: (ctx) => !!handleFor(ctx),
    run: (ctx) => void handleFor(ctx)?.paste(),
  },
  {
    id: 'terminal.selectAll',
    label: 'Select All',
    icon: 'list',
    group: 'clipboard',
    order: 30,
    when: (ctx) => ctx.scope === 'terminal',
    enabled: (ctx) => !!handleFor(ctx),
    run: (ctx) => handleFor(ctx)?.selectAll(),
  },

  // ── view ──────────────────────────────────────────────────────────────────
  {
    id: 'terminal.clear',
    label: 'Clear',
    icon: 'refresh',
    group: 'view',
    order: 40,
    when: (ctx) => ctx.scope === 'terminal',
    enabled: (ctx) => !!handleFor(ctx),
    run: (ctx) => handleFor(ctx)?.clear(),
  },

  // ── session ─────────────────────────────────────────────────────────────────
  {
    id: 'terminal.new',
    label: 'New Terminal',
    icon: 'plus',
    group: 'session',
    order: 50,
    when: (ctx) => ctx.scope === 'terminal',
    run: () => void createSession(),
  },
  {
    id: 'terminal.split',
    label: 'Split Terminal',
    icon: 'open-in-window',
    group: 'session',
    order: 60,
    when: (ctx) => ctx.scope === 'terminal',
    run: () => void createSession(),
  },
  {
    id: 'terminal.kill',
    label: 'Kill Terminal',
    icon: 'xmark',
    group: 'session',
    order: 70,
    when: (ctx) => ctx.scope === 'terminal',
    enabled: (ctx) => !!ctx.terminalId,
    run: (ctx) => {
      if (ctx.terminalId) killTerminal(ctx.terminalId)
    },
  },
]

/** Register the terminal action set into the shared registry (called at init). */
export function registerTerminalActions(): void {
  registry.registerAll(terminalActions)
}
