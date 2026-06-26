/**
 * Editor + editor-tab context actions (Requirement 6).
 *
 * Clipboard, Command Palette, Copy File Path, Close Editor and the tab-close
 * family are fully wired. The LSP-backed operations (Format Document, Rename
 * Symbol, Go to Definition, Find References) are *gated* on a connected language
 * server for the file (`enabled`) so they disable — not hide — when LSP is
 * unavailable (Requirement 6.2 / 8.1). Their edit backends were out of M6's
 * scope (diagnostics + completion only), so their `run` degrades to a non-
 * blocking notice rather than crashing (Strict Rule 6). Wiring them is a
 * straightforward follow-up once the engine exposes the LSP edit requests.
 *
 * Orders increase across groups so the registry renders them in design order.
 */
import { get } from 'svelte/store'
import { registry } from '../context-menu/actionRegistry'
import type { ContextAction, ContextMenuContext } from '../context-menu/contextTypes'
import { activeDoc, editorCut, editorCopy, editorPaste } from '../editor/active-editor'
import { openPalette } from '../stores/ui'
import {
  closeActiveTab,
  closeTabById,
  closeOtherTabs,
  closeTabsToRight,
  closeSavedTabs,
  isEditorTab,
  tabs,
} from '../stores/tabs'
import { explainSelection, fixSelection, generateTestsForSelection } from '../stores/ai-actions'
import { openSimpleDiff } from '../stores/simple-diff'
import { expandPanel } from '../stores/panels'
import { requestTreeReveal } from '../stores/file-tree'
import { lsp } from '../stores/lsp'

/** A connected language server exists for the file the menu was opened on. */
function isLspConnected(ctx: ContextMenuContext): boolean {
  if (!ctx.path) return false
  const status = get(lsp).status[ctx.path]
  return !!status && status.state === 'connected'
}

/** Graceful placeholder for an LSP edit feature whose engine command is pending. */
function lspFeaturePending(name: string): void {
  console.info(`[GwenLand] "${name}" needs an LSP edit feature that isn't wired yet.`)
}

function previewUnsavedDiff(): void {
  const state = get(tabs)
  const tab = state.tabs.find((candidate) => candidate.id === state.activeId)
  if (!tab || !isEditorTab(tab)) return
  const current = activeDoc() ?? tab.state.doc.toString()
  if (current === tab.baseline) {
    alert('No unsaved changes.')
    return
  }
  openSimpleDiff({
    title: 'Unsaved Changes',
    leftLabel: 'Saved',
    rightLabel: 'Current',
    left: tab.baseline,
    right: current,
  })
}

const editorActions: ContextAction[] = [
  // ── clipboard ─────────────────────────────────────────────────────────────
  {
    id: 'editor.cut',
    label: 'Cut',
    icon: 'scissor',
    group: 'clipboard',
    order: 10,
    shortcut: 'Ctrl+X',
    when: (ctx) => ctx.scope === 'editor',
    enabled: (ctx) => !!ctx.selectionText,
    run: () => void editorCut(),
  },
  {
    id: 'editor.copy',
    label: 'Copy',
    icon: 'copy',
    group: 'clipboard',
    order: 20,
    shortcut: 'Ctrl+C',
    when: (ctx) => ctx.scope === 'editor',
    enabled: (ctx) => !!ctx.selectionText,
    run: () => void editorCopy(),
  },
  {
    id: 'editor.paste',
    label: 'Paste',
    icon: 'clipboard-check',
    group: 'clipboard',
    order: 30,
    shortcut: 'Ctrl+V',
    when: (ctx) => ctx.scope === 'editor',
    run: () => void editorPaste(),
  },

  // ── format ──────────────────────────────────────────────────────────────
  {
    id: 'editor.format',
    label: 'Format Document',
    icon: 'magic-wand',
    group: 'format',
    order: 40,
    when: (ctx) => ctx.scope === 'editor',
    enabled: isLspConnected,
    run: () => lspFeaturePending('Format Document'),
  },

  // ── lsp ─────────────────────────────────────────────────────────────────
  {
    id: 'editor.renameSymbol',
    label: 'Rename Symbol',
    icon: 'text',
    group: 'lsp',
    order: 50,
    shortcut: 'F2',
    when: (ctx) => ctx.scope === 'editor',
    enabled: isLspConnected,
    run: () => lspFeaturePending('Rename Symbol'),
  },
  {
    id: 'editor.goToDefinition',
    label: 'Go to Definition',
    icon: 'code',
    group: 'lsp',
    order: 60,
    shortcut: 'F12',
    when: (ctx) => ctx.scope === 'editor',
    enabled: isLspConnected,
    run: () => lspFeaturePending('Go to Definition'),
  },
  {
    id: 'editor.findReferences',
    label: 'Find References',
    icon: 'list',
    group: 'lsp',
    order: 70,
    when: (ctx) => ctx.scope === 'editor',
    enabled: isLspConnected,
    run: () => lspFeaturePending('Find References'),
  },

  // ── navigate ──────────────────────────────────────────────────────────────
  {
    id: 'editor.commandPalette',
    label: 'Command Palette',
    icon: 'search',
    group: 'navigate',
    order: 80,
    commandId: 'palette.open',
    when: (ctx) => ctx.scope === 'editor',
    run: () => openPalette(),
  },
  {
    id: 'editor.copyFilePath',
    label: 'Copy File Path',
    icon: 'copy',
    group: 'navigate',
    order: 90,
    when: (ctx) => ctx.scope === 'editor',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) void navigator.clipboard.writeText(ctx.path)
    },
  },
  {
    id: 'editor.aiExplainSelection',
    label: 'AI: Explain Selection',
    icon: 'sparks',
    group: 'ai',
    order: 95,
    commandId: 'ai.explainSelection',
    when: (ctx) => ctx.scope === 'editor',
    run: explainSelection,
  },
  {
    id: 'editor.aiFixSelection',
    label: 'AI: Fix Selection',
    icon: 'magic-wand',
    group: 'ai',
    order: 96,
    commandId: 'ai.fixSelection',
    when: (ctx) => ctx.scope === 'editor',
    run: fixSelection,
  },
  {
    id: 'editor.aiGenerateTests',
    label: 'AI: Generate Tests',
    icon: 'check',
    group: 'ai',
    order: 97,
    commandId: 'ai.generateTests',
    when: (ctx) => ctx.scope === 'editor',
    run: generateTestsForSelection,
  },
  {
    id: 'editor.previewUnsavedDiff',
    label: 'Safety: Preview Diff',
    icon: 'warning-circle',
    group: 'safety',
    order: 98,
    when: (ctx) => ctx.scope === 'editor',
    run: previewUnsavedDiff,
  },

  // ── layout ──────────────────────────────────────────────────────────────
  {
    id: 'editor.close',
    label: 'Close Editor',
    icon: 'xmark',
    group: 'layout',
    order: 110,
    commandId: 'tab.close',
    when: (ctx) => ctx.scope === 'editor',
    run: () => closeActiveTab(),
  },
]

const tabActions: ContextAction[] = [
  // ── close ─────────────────────────────────────────────────────────────────
  {
    id: 'tab.close',
    label: 'Close',
    icon: 'xmark',
    group: 'close',
    order: 10,
    commandId: 'tab.close',
    when: (ctx) => ctx.scope === 'editor_tab',
    enabled: (ctx) => !!ctx.tabId,
    run: (ctx) => {
      if (ctx.tabId) closeTabById(ctx.tabId)
    },
  },
  {
    id: 'tab.closeOthers',
    label: 'Close Others',
    icon: 'xmark-circle',
    group: 'close',
    order: 20,
    when: (ctx) => ctx.scope === 'editor_tab',
    enabled: (ctx) => !!ctx.tabId,
    run: (ctx) => {
      if (ctx.tabId) closeOtherTabs(ctx.tabId)
    },
  },
  {
    id: 'tab.closeToRight',
    label: 'Close to Right',
    icon: 'arrow-right',
    group: 'close',
    order: 30,
    when: (ctx) => ctx.scope === 'editor_tab',
    enabled: (ctx) => !!ctx.tabId,
    run: (ctx) => {
      if (ctx.tabId) closeTabsToRight(ctx.tabId)
    },
  },
  {
    id: 'tab.closeSaved',
    label: 'Close Saved',
    icon: 'check',
    group: 'close',
    order: 40,
    when: (ctx) => ctx.scope === 'editor_tab',
    run: () => closeSavedTabs(),
  },

  // ── navigate ──────────────────────────────────────────────────────────────
  {
    id: 'tab.revealInFileTree',
    label: 'Reveal in File Tree',
    icon: 'eye',
    group: 'navigate',
    order: 50,
    when: (ctx) => ctx.scope === 'editor_tab',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (!ctx.path) return
      expandPanel('fileTree')
      requestTreeReveal(ctx.path)
    },
  },
  {
    id: 'tab.copyPath',
    label: 'Copy Path',
    icon: 'copy',
    group: 'navigate',
    order: 60,
    when: (ctx) => ctx.scope === 'editor_tab',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) void navigator.clipboard.writeText(ctx.path)
    },
  },

]

/** Register the editor + tab action sets into the shared registry (called at init). */
export function registerEditorActions(): void {
  registry.registerAll(editorActions)
  registry.registerAll(tabActions)
}
