<script lang="ts">
  import { onDestroy } from 'svelte'
  import { get } from 'svelte/store'
  import { appActive } from '../stores/app-focus'
  import { tabs, persistTabState, recomputeDirty, isEditorTab } from '../stores/tabs'
  import { setCursorFromState, clearCursor } from '../stores/cursor'
  import { setActiveEditor } from '../editor/active-editor'
  import { lsp, lspChangePath, languageForPath } from '../stores/lsp'
  import { openContextMenuSmart } from '../context-menu/globalContextMenu'
  import { diffReview, acceptHunk, rejectHunk, sameFilePath } from '../stores/diff-review'
  import { applyReviewOverlay, clearReviewOverlay } from '../editor/diff-overlay'
  import {
    createEditorState,
    mountEditorView,
    applyDiagnostics,
    EditorView,
  } from '../editor/codemirror-setup'

  let host: HTMLDivElement
  let view: EditorView | null = null
  // The tab id currently mounted in `view` — drives swap-on-switch.
  let mountedId: string | null = null
  // The mounted tab's file path (for routing LSP changes/diagnostics).
  let mountedPath: string | null = null
  // Debounce handle for didChange notifications (Requirement 9.8).
  let changeTimer: ReturnType<typeof setTimeout> | null = null

  /** Snapshot the live view back into the store for the tab it belongs to. */
  function persistMounted() {
    if (view && mountedId) persistTabState(mountedId, view.state)
  }

  /** Debounce a full-text didChange for the mounted document. */
  function scheduleLspChange() {
    if (changeTimer) clearTimeout(changeTimer)
    changeTimer = setTimeout(() => {
      changeTimer = null
      if (view && mountedPath) void lspChangePath(mountedPath, view.state.doc.toString())
    }, 250)
  }

  /** Send any pending change immediately (before a switch/teardown). */
  function flushLspChange() {
    if (!changeTimer) return
    clearTimeout(changeTimer)
    changeTimer = null
    if (view && mountedPath) void lspChangePath(mountedPath, view.state.doc.toString())
  }

  /** Push the current store diagnostics for the mounted path into the view. */
  function applyDiagnosticsToView() {
    if (!view || mountedPath === null) return
    applyDiagnostics(view, get(lsp).diagnostics[mountedPath] ?? [])
  }

  /** Drive the diff-review overlay for the mounted file (Req 12). */
  function updateReviewOverlay() {
    if (!view) return
    const st = get(diffReview)
    const file =
      st.active && mountedPath ? st.files.find((f) => sameFilePath(f.absPath, mountedPath)) : null
    if (file) applyReviewOverlay(view, file, { onAccept: acceptHunk, onReject: rejectHunk })
    else clearReviewOverlay(view)
  }

  /** Tear down the current view and mount the given tab's stored state. */
  function mountTab(id: string) {
    if (view) {
      view.destroy()
      view = null
    }
    const tab = get(tabs).tabs.find((t) => t.id === id)
    // Defensive: the Editor is only rendered for an editor-kind active tab, but
    // guard anyway so a preview tab can never be treated as a document.
    if (!tab || !isEditorTab(tab) || !host) {
      mountedId = null
      mountedPath = null
      setActiveEditor(null)
      clearCursor()
      return
    }
    // Rebuild state with an onDocChange listener bound to THIS tab so edits
    // recompute dirty against this tab's baseline. Seed from the tab's stored
    // doc so cursor/scroll/undo are preserved across switches.
    const doc = tab.state.doc.toString()
    const onDocChange = () => {
      if (view) recomputeDirty(id, view.state.doc.toString())
      scheduleLspChange()
    }
    // Mirror this tab's cursor into the status bar on every selection change.
    const onSelectionChange = () => {
      if (view) setCursorFromState(view.state)
    }
    const state = createEditorState(doc, onDocChange, onSelectionChange, tab.path)
    view = mountEditorView(state, host)
    mountedId = id
    mountedPath = tab.path
    setActiveEditor(view)
    // Seed the status bar with the just-mounted tab's cursor position.
    setCursorFromState(view.state)
    // Seed any diagnostics already known for this document.
    applyDiagnosticsToView()
    // Seed the diff-review overlay if this file is part of an active review.
    updateReviewOverlay()
    view.focus()
  }

  // React to active-tab changes: persist the outgoing tab, mount the incoming.
  // Using $effect so it runs after host is bound and on every activeId change.
  $effect(() => {
    const activeId = $tabs.activeId
    if (activeId === mountedId) return
    // Flush + persist whatever is currently mounted before switching away.
    flushLspChange()
    persistMounted()
    if (activeId) mountTab(activeId)
    else {
      if (view) {
        view.destroy()
        view = null
      }
      mountedId = null
      mountedPath = null
      setActiveEditor(null)
      clearCursor()
    }
  })

  // Re-apply diagnostics whenever the LSP store updates (server pushed new ones
  // for the mounted document, or cleared them).
  $effect(() => {
    void $lsp
    applyDiagnosticsToView()
  })

  // Re-render the diff-review overlay whenever the review session changes
  // (hunk accepted/rejected, active file switched, or review exited).
  $effect(() => {
    void $diffReview
    updateReviewOverlay()
  })

  onDestroy(() => {
    flushLspChange()
    persistMounted()
    if (changeTimer) clearTimeout(changeTimer)
    if (view) view.destroy()
    setActiveEditor(null)
    clearCursor()
  })

  // Right-click opens the editor context menu (M9). Selection + language id are
  // captured from the live view so selection/LSP-aware actions gate correctly.
  function onEditorContextMenu(e: MouseEvent) {
    if (!view) return
    const sel = view.state.selection.main
    const path = mountedPath || undefined
    // Smart routing: the search-panel <input>s get the input menu; the code
    // surface (contenteditable, not an <input>) gets the editor menu.
    openContextMenuSmart(e, {
      scope: 'editor',
      path,
      languageId: path ? (languageForPath(path) ?? undefined) : undefined,
      selectionText: sel.empty ? undefined : view.state.sliceDoc(sel.from, sel.to),
    })
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- Background throttle: `inactive` halts the cursor-blink animation (a
     repaint loop) while the window is unfocused/hidden. No behavior change. -->
<div
  class="editor-host"
  class:inactive={!$appActive}
  bind:this={host}
  oncontextmenu={onEditorContextMenu}
></div>

<style>
  .editor-host {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    background-color: var(--background);
    color: var(--foreground);
    font-family: var(--font-mono);
    font-size: 13px;
  }
  /* CM6 chrome theming — ported from the legacy #cm-host rules. */
  .editor-host :global(.cm-editor) {
    height: 100%;
    position: relative;
    background-color: var(--background);
    color: var(--foreground);
  }
  .editor-host :global(.cm-editor.cm-focused) {
    outline: none;
  }
  /* Background throttle: stop the cursor-blink animation while the window is
     inactive so CodeMirror isn't repainting the caret in the background. */
  .editor-host.inactive :global(.cm-cursorLayer) {
    animation: none !important;
  }
  .editor-host :global(.cm-scroller) {
    font-family: var(--font-mono);
    line-height: 1.6;
  }
  .editor-host :global(.cm-content) {
    caret-color: var(--primary);
    padding: 6px 0;
  }
  .editor-host :global(.cm-cursor) {
    border-left: 2px solid var(--primary);
  }
  .editor-host :global(.cm-gutters) {
    background-color: var(--background);
    color: var(--muted-foreground);
    border-right: 1px solid var(--border);
  }
  .editor-host :global(.cm-lineNumbers .cm-gutterElement) {
    padding: 0 8px 0 12px;
    color: var(--muted-foreground);
  }
  .editor-host :global(.cm-activeLine) {
    background-color: color-mix(in srgb, var(--sidebar-accent) 60%, transparent);
  }
  .editor-host :global(.cm-activeLineGutter) {
    background-color: var(--sidebar-accent);
    color: var(--foreground);
  }
  .editor-host :global(.cm-selectionBackground),
  .editor-host :global(.cm-content ::selection) {
    background-color: color-mix(in srgb, var(--primary) 28%, transparent) !important;
  }
  .editor-host :global(.cm-focused .cm-selectionBackground) {
    background-color: color-mix(in srgb, var(--primary) 36%, transparent) !important;
  }
  .editor-host :global(.cm-selectionMatch) {
    background-color: color-mix(in srgb, var(--chart-2) 30%, transparent);
    border-radius: 2px;
  }
  .editor-host :global(.cm-matchingBracket) {
    background-color: color-mix(in srgb, var(--primary) 25%, transparent);
    color: var(--foreground) !important;
    border-radius: 2px;
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--primary) 50%, transparent);
  }
  .editor-host :global(.cm-nonmatchingBracket) {
    background-color: color-mix(in srgb, var(--destructive) 30%, transparent);
    border-radius: 2px;
  }
  .editor-host :global(.cm-tooltip) {
    background-color: var(--popover);
    color: var(--popover-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-lg);
  }
  /* M6 LSP diagnostics — tooltip + gutter markers themed to match the editor. */
  .editor-host :global(.cm-tooltip-lint) {
    background-color: var(--popover);
    color: var(--popover-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  .editor-host :global(.cm-diagnostic) {
    font-family: var(--font-sans);
    font-size: 12px;
    padding: 4px 8px 4px 10px;
  }
  .editor-host :global(.cm-diagnostic-error) {
    border-left: 4px solid var(--destructive);
  }
  .editor-host :global(.cm-diagnostic-warning) {
    border-left: 4px solid var(--chart-4, #d29922);
  }
  .editor-host :global(.cm-diagnostic-info),
  .editor-host :global(.cm-diagnostic-hint) {
    border-left: 4px solid var(--primary);
  }
  .editor-host :global(.cm-lintRange-error) {
    text-decoration-color: var(--destructive);
  }
  .editor-host :global(.cm-gutter-lint) {
    width: 0.8em;
  }

  /* ── M8 diff-review overlay (Req 12.2-12.5) ──────────────────────────────
     Diff colors live here (literal M8 values) because the editor is outside
     the AI pane where the --ai-* tokens are scoped. */
  .editor-host :global(.cm-diff-removed-line) {
    background-color: rgba(220, 53, 69, 0.12);
  }
  .editor-host :global(.cm-diff-gutter-removed) {
    background-color: rgba(220, 53, 69, 0.25);
  }
  .editor-host :global(.cm-diff-widget) {
    font-family: var(--font-mono);
    font-size: 12px;
    border-left: 2px solid var(--primary);
    margin: 2px 0;
  }
  .editor-host :global(.cm-diff-added-row) {
    background-color: rgba(40, 167, 69, 0.12);
    box-shadow: inset 3px 0 0 rgba(40, 167, 69, 0.25);
    padding: 0 8px 0 12px;
    white-space: pre-wrap;
    color: var(--foreground);
  }
  .editor-host :global(.cm-diff-bar) {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px 4px 12px;
    background-color: color-mix(in srgb, var(--primary) 10%, var(--background));
    font-family: var(--font-sans);
  }
  .editor-host :global(.cm-diff-bar-status) {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    color: var(--muted-foreground);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .editor-host :global(.cm-diff-failed .cm-diff-bar-status) {
    color: var(--destructive, #e06c75);
  }
  .editor-host :global(.cm-diff-btn) {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 10px;
    border-radius: 999px;
    border: 1px solid transparent;
    cursor: pointer;
  }
  .editor-host :global(.cm-diff-btn-accept) {
    color: #fff;
    background-color: rgba(40, 167, 69, 0.85);
  }
  .editor-host :global(.cm-diff-btn-reject) {
    color: var(--foreground);
    background-color: transparent;
    border-color: var(--border);
  }
</style>
