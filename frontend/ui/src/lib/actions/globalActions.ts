/**
 * Input + global fallback context actions (M9 follow-up).
 *
 * `input` scope: Cut/Copy/Paste/Select All for any text field anywhere in the
 * app (settings, command palette, search, AI composer, …) — VS Code-style.
 * `global` scope: the window-level fallback so a right-click on any otherwise
 * unhandled pane still gets a useful menu (the IDE menu is the default
 * everywhere, never the native one).
 */
import { registry } from '../context-menu/actionRegistry'
import type { ContextAction } from '../context-menu/contextTypes'
import {
  inputCopy,
  inputCut,
  inputPaste,
  inputSelectAll,
  inputHasSelection,
  inputHasValue,
} from '../context-menu/globalContextMenu'
import { openPalette, openSettings } from '../stores/ui'
import { togglePanel } from '../stores/panels'
import { toggleAiChat } from '../stores/ai-chat'

const inputActions: ContextAction[] = [
  {
    id: 'input.cut',
    label: 'Cut',
    icon: 'scissor',
    group: 'clipboard',
    order: 10,
    shortcut: 'Ctrl+X',
    when: (ctx) => ctx.scope === 'input',
    enabled: () => inputHasSelection(),
    run: () => void inputCut(),
  },
  {
    id: 'input.copy',
    label: 'Copy',
    icon: 'copy',
    group: 'clipboard',
    order: 20,
    shortcut: 'Ctrl+C',
    when: (ctx) => ctx.scope === 'input',
    enabled: () => inputHasSelection(),
    run: () => void inputCopy(),
  },
  {
    id: 'input.paste',
    label: 'Paste',
    icon: 'clipboard-check',
    group: 'clipboard',
    order: 30,
    shortcut: 'Ctrl+V',
    when: (ctx) => ctx.scope === 'input',
    run: () => void inputPaste(),
  },
  {
    id: 'input.selectAll',
    label: 'Select All',
    icon: 'list',
    group: 'select',
    order: 40,
    shortcut: 'Ctrl+A',
    when: (ctx) => ctx.scope === 'input',
    enabled: () => inputHasValue(),
    run: () => inputSelectAll(),
  },
]

const globalActions: ContextAction[] = [
  {
    id: 'global.copy',
    label: 'Copy',
    icon: 'copy',
    group: 'clipboard',
    order: 10,
    shortcut: 'Ctrl+C',
    when: (ctx) => ctx.scope === 'global',
    enabled: (ctx) => !!ctx.selectionText,
    run: (ctx) => {
      if (ctx.selectionText) void navigator.clipboard.writeText(ctx.selectionText).catch(() => {})
    },
  },
  {
    id: 'global.commandPalette',
    label: 'Command Palette',
    icon: 'search',
    group: 'view',
    order: 20,
    commandId: 'palette.open',
    when: (ctx) => ctx.scope === 'global',
    run: () => openPalette(),
  },
  {
    id: 'global.toggleExplorer',
    label: 'Toggle Explorer',
    icon: 'page',
    group: 'view',
    order: 30,
    commandId: 'panel.explorer',
    when: (ctx) => ctx.scope === 'global',
    run: () => togglePanel('fileTree'),
  },
  {
    id: 'global.toggleTerminal',
    label: 'Toggle Terminal',
    icon: 'terminal',
    group: 'view',
    order: 40,
    commandId: 'panel.terminal',
    when: (ctx) => ctx.scope === 'global',
    run: () => togglePanel('terminal'),
  },
  {
    id: 'global.toggleAi',
    label: 'Toggle AI Panel',
    icon: 'sparks',
    group: 'view',
    order: 50,
    when: (ctx) => ctx.scope === 'global',
    run: () => toggleAiChat(),
  },
  {
    id: 'global.settings',
    label: 'Settings',
    icon: 'settings',
    group: 'tools',
    order: 60,
    commandId: 'settings.open',
    when: (ctx) => ctx.scope === 'global',
    run: () => openSettings(),
  },
]

/** Register the input + global fallback action sets (called at init). */
export function registerGlobalActions(): void {
  registry.registerAll(inputActions)
  registry.registerAll(globalActions)
}
