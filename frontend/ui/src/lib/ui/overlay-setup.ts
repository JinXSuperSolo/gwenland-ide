/**
 * Wires every app overlay into the centralized Escape stack (M-keynav §2).
 *
 * Each overlay keeps owning its open/close store; here we register its close
 * handler once and mirror its `open` flag into the stack via a subscription, so
 * `closeTopmost()` always knows the live stack order. Called once at app start
 * (App.svelte onMount). Returns a disposer that unsubscribes + unregisters.
 *
 * Priority/order is purely temporal: whichever overlay opened most recently is
 * closed first, which matches what the user perceives as "topmost".
 */
import type { Unsubscriber } from 'svelte/store'
import { registerOverlay, syncOverlay } from '../stores/overlay-stack'
import {
  paletteOpen,
  closePalette,
  settingsOpen,
  closeSettings,
  aboutOpen,
  closeAbout,
  changelogOpen,
  closeChangelog,
} from '../stores/ui'
import { promptDialog, cancelPrompt } from '../stores/prompt-dialog'
import { contextMenuStore, closeContextMenu } from '../context-menu/contextMenuStore'
import { gitGraphWindow, closeGitGraphWindow } from '../stores/gitGraphWindow'
import { treeInput, cancelTreeInput } from '../stores/tree-input'

interface OverlayWiring {
  id: string
  close: () => void
  /** Subscribe to the overlay's store and report its open boolean. */
  subscribe: (report: (isOpen: boolean) => void) => Unsubscriber
}

const OVERLAYS: OverlayWiring[] = [
  {
    id: 'command-palette',
    close: closePalette,
    subscribe: (report) => paletteOpen.subscribe((v) => report(v)),
  },
  {
    id: 'settings',
    close: closeSettings,
    subscribe: (report) => settingsOpen.subscribe((v) => report(v)),
  },
  {
    id: 'about',
    close: closeAbout,
    subscribe: (report) => aboutOpen.subscribe((v) => report(v)),
  },
  {
    id: 'changelog',
    close: closeChangelog,
    subscribe: (report) => changelogOpen.subscribe((v) => report(v)),
  },
  {
    id: 'prompt',
    close: cancelPrompt,
    subscribe: (report) => promptDialog.subscribe((s) => report(s.open)),
  },
  {
    id: 'context-menu',
    close: closeContextMenu,
    subscribe: (report) => contextMenuStore.subscribe((s) => report(s.open)),
  },
  {
    id: 'git-graph',
    close: closeGitGraphWindow,
    subscribe: (report) => gitGraphWindow.subscribe((s) => report(s.open)),
  },
  {
    id: 'tree-input',
    close: cancelTreeInput,
    subscribe: (report) => treeInput.subscribe((s) => report(s.open)),
  },
]

/** Register all overlays and start mirroring their open state. */
export function initOverlayStack(): () => void {
  const disposers: Array<() => void> = []
  for (const overlay of OVERLAYS) {
    disposers.push(registerOverlay({ id: overlay.id, close: overlay.close }))
    disposers.push(overlay.subscribe((isOpen) => syncOverlay(overlay.id, isOpen)))
  }
  return () => {
    for (const dispose of disposers) dispose()
  }
}
