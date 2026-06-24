/**
 * Central wiring hub (GWEN-241 / GWEN-243 / GWEN-288). Registers the global
 * command/shortcut set and exposes the menu-bar descriptor. Items that were
 * stubs/disabled in the legacy version stay stubs here — Wave 5 ports behavior,
 * it does not grow scope.
 */
import { registerCommand } from './stores/commands'
import { openFolder } from './stores/workspace'
import { saveActiveTab, closeActiveTab, cycleTab, newUntitledFile } from './stores/tabs'
import { togglePanel } from './stores/panels'
import { openPalette, openSettings } from './stores/ui'
import {
  hasActiveEditor,
  editorUndo,
  editorRedo,
  editorFind,
  editorSelectAll,
} from './editor/active-editor'

export interface MenuItem {
  label?: string
  shortcut?: string
  action?: () => void
  disabled?: boolean
  type?: 'divider'
  /** A submenu rendered lazily (e.g. Open Recent). */
  children?: 'recent'
}

/** Register the startup command set (palette + keyboard shortcuts). */
export function registerCommands(): void {
  registerCommand('file.new', 'New File', ['Ctrl+N', 'Meta+N'], () => newUntitledFile())
  registerCommand('file.save', 'Save', ['Ctrl+S', 'Meta+S'], () => void saveActiveTab())
  registerCommand('file.openFolder', 'Open Folder', ['Ctrl+Shift+O', 'Meta+Shift+O'], () =>
    void openFolder(),
  )
  registerCommand('tab.close', 'Close Tab', ['Ctrl+W', 'Meta+W'], () => closeActiveTab())
  registerCommand('tab.next', 'Next Tab', ['Ctrl+Tab'], () => cycleTab(1))
  registerCommand('tab.prev', 'Previous Tab', ['Ctrl+Shift+Tab'], () => cycleTab(-1))
  registerCommand('palette.open', 'Command Palette', ['Ctrl+Shift+P', 'Meta+Shift+P'], () =>
    openPalette(),
  )
  registerCommand('search.inFile', 'Find in File', ['Ctrl+F', 'Meta+F'], () => editorFind())
  registerCommand('panel.explorer', 'Toggle Explorer', ['Ctrl+B', 'Meta+B'], () =>
    togglePanel('fileTree'),
  )
  registerCommand('panel.terminal', 'Toggle Terminal', ['Ctrl+J'], () =>
    togglePanel('terminal'),
  )
  registerCommand('settings.open', 'Open Settings', ['Ctrl+Shift+,', 'Meta+Shift+,'], () =>
    openSettings(),
  )
}

// Edit/Selection actions guard on an editor being open (matches legacy withEditor).
function withEditor(fn: () => void): () => void {
  return () => {
    if (!hasActiveEditor()) return
    fn()
  }
}

// ── Menu descriptors (GWEN-288). Disabled stubs preserved from the legacy. ──

const fileMenu: MenuItem[] = [
  { label: 'New Text File', shortcut: 'Ctrl+N', action: () => newUntitledFile() },
  { label: 'New File...', disabled: true },
  { label: 'New Window', shortcut: 'Ctrl+Shift+N', disabled: true },
  { type: 'divider' },
  { label: 'Open File...', shortcut: 'Ctrl+O', disabled: true },
  { label: 'Open Folder...', shortcut: 'Ctrl+K Ctrl+O', action: () => void openFolder() },
  { label: 'Open Recent', children: 'recent' },
  { type: 'divider' },
  { label: 'Save', shortcut: 'Ctrl+S', action: () => void saveActiveTab() },
  { label: 'Save All', shortcut: 'Ctrl+K S', disabled: true },
  { type: 'divider' },
  { label: 'Auto Save', disabled: true },
  { label: 'Preferences', disabled: true },
  { type: 'divider' },
  { label: 'Close Editor', shortcut: 'Ctrl+F4', action: () => closeActiveTab() },
  { label: 'Close Folder', shortcut: 'Ctrl+K F', disabled: true },
]

const editMenu: MenuItem[] = [
  { label: 'Undo', shortcut: 'Ctrl+Z', action: withEditor(editorUndo) },
  { label: 'Redo', shortcut: 'Ctrl+Y', action: withEditor(editorRedo) },
  { type: 'divider' },
  { label: 'Cut', shortcut: 'Ctrl+X', disabled: true },
  { label: 'Copy', shortcut: 'Ctrl+C', disabled: true },
  { label: 'Paste', shortcut: 'Ctrl+V', disabled: true },
  { type: 'divider' },
  { label: 'Find', shortcut: 'Ctrl+F', action: withEditor(editorFind) },
  { label: 'Replace', shortcut: 'Ctrl+H', action: withEditor(editorFind) },
]

const selectionMenu: MenuItem[] = [
  { label: 'Select All', shortcut: 'Ctrl+A', action: withEditor(editorSelectAll) },
  { label: 'Expand Selection', shortcut: 'Shift+Alt+Right', disabled: true },
  { label: 'Shrink Selection', shortcut: 'Shift+Alt+Left', disabled: true },
]

const viewMenu: MenuItem[] = [
  { label: 'Command Palette...', shortcut: 'Ctrl+Shift+P', action: () => openPalette() },
  { type: 'divider' },
  { label: 'Toggle Explorer', shortcut: 'Ctrl+B', action: () => togglePanel('fileTree') },
  { label: 'Toggle Terminal', shortcut: 'Ctrl+J', action: () => togglePanel('terminal') },
  { type: 'divider' },
  { label: 'Settings', shortcut: 'Ctrl+Shift+,', action: () => openSettings() },
]

const goMenu: MenuItem[] = [
  { label: 'Go to File...', shortcut: 'Ctrl+P', action: () => openPalette() },
  { label: 'Go to Line...', shortcut: 'Ctrl+G', disabled: true },
]

const runMenu: MenuItem[] = [
  { label: 'Run Without Debugging', shortcut: 'Ctrl+F5', disabled: true },
  { label: 'Start Debugging', shortcut: 'F5', disabled: true },
  { label: 'Stop', shortcut: 'Shift+F5', disabled: true },
]

const terminalMenu: MenuItem[] = [
  { label: 'New Terminal', disabled: true },
  { label: 'Kill Terminal', disabled: true },
  { label: 'Clear Terminal', disabled: true },
]

const helpMenu: MenuItem[] = [
  {
    label: 'About GwenLand IDE',
    action: () =>
      alert('GwenLand IDE\nVersion 0.1.0\n\nA lightweight, local-first code editor built with Tauri + Rust.'),
  },
  { label: 'Open Repository', disabled: true },
]

/** Titlebar menu descriptor, in display order. */
export const MENUS: { name: string; items: MenuItem[] }[] = [
  { name: 'File', items: fileMenu },
  { name: 'Edit', items: editMenu },
  { name: 'Selection', items: selectionMenu },
  { name: 'View', items: viewMenu },
  { name: 'Go', items: goMenu },
  { name: 'Run', items: runMenu },
  { name: 'Terminal', items: terminalMenu },
  { name: 'Help', items: helpMenu },
]
