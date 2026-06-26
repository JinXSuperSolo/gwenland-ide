<script lang="ts">
  import { onDestroy } from 'svelte'
  import { get } from 'svelte/store'
  import { appActive } from '../stores/app-focus'
  import {
    tabs,
    persistTabState,
    recomputeDirty,
    isEditorTab,
    openFile,
    setActiveGroup,
  } from '../stores/tabs'
  import { workspace } from '../stores/workspace'
  import { setCursorFromState, clearCursor } from '../stores/cursor'
  import { editorGoToDefinitionAt, setActiveEditor } from '../editor/active-editor'
  import { lsp, lspChangePath, languageForPath } from '../stores/lsp'
  import { editorPreferences } from '../stores/editor-preferences'
  import { openContextMenuSmart } from '../context-menu/globalContextMenu'
  import { diffReview, acceptHunk, rejectHunk, sameFilePath } from '../stores/diff-review'
  import { applyReviewOverlay, clearReviewOverlay } from '../editor/diff-overlay'
  import MarkdownPreview from './MarkdownPreview.svelte'
  import {
    createEditorState,
    mountEditorView,
    applyDiagnostics,
    EditorView,
  } from '../editor/codemirror-setup'

  let host: HTMLDivElement
  let {
    tabId = null,
    groupId = null,
    active = true,
  }: { tabId?: string | null; groupId?: string | null; active?: boolean } = $props()
  let view: EditorView | null = null
  // The tab id currently mounted in `view` — drives swap-on-switch.
  let mountedId: string | null = null
  // The mounted tab's file path (for routing LSP changes/diagnostics).
  let mountedPath = $state<string | null>(null)
  // Debounce handle for didChange notifications (Requirement 9.8).
  let changeTimer: ReturnType<typeof setTimeout> | null = null
  let stickyScope = $state('')
  let scrollDom: HTMLElement | null = null
  let minimapCanvas = $state<HTMLCanvasElement | null>(null)
  let minimapFrame = 0
  let minimapDragging = false
  let markdownPreviewText = $state('')
  let markdownTimer: ReturnType<typeof setTimeout> | null = null

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

  function scheduleMarkdownPreview() {
    if (markdownTimer) clearTimeout(markdownTimer)
    markdownTimer = setTimeout(() => {
      markdownTimer = null
      markdownPreviewText = view?.state.doc.toString() ?? ''
    }, 300)
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

  function scopeLabel(line: string): string | null {
    const trimmed = line.trim()
    let match = trimmed.match(/^(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$][\w$]*)/)
    if (match) return `function ${match[1]}`
    match = trimmed.match(/^(?:export\s+)?(class|interface|type|enum|struct|trait|impl)\s+([A-Za-z_$][\w$]*)/)
    if (match) return `${match[1]} ${match[2]}`
    match = trimmed.match(/^(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_$][\w$]*)/)
    if (match) return `fn ${match[1]}`
    match = trimmed.match(/^(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)\s*=>/)
    if (match) return `function ${match[1]}`
    return null
  }

  function updateStickyScope() {
    if (!view) {
      stickyScope = ''
      return
    }
    const topLine = view.state.doc.lineAt(view.viewport.from).number
    const limit = Math.max(1, topLine - 160)
    for (let lineNo = topLine; lineNo >= limit; lineNo -= 1) {
      const label = scopeLabel(view.state.doc.line(lineNo).text)
      if (label) {
        stickyScope = label
        return
      }
    }
    stickyScope = ''
  }

  function onEditorScroll() {
    updateStickyScope()
    scheduleMinimapDraw()
  }

  function scheduleMinimapDraw() {
    if (minimapFrame) return
    minimapFrame = requestAnimationFrame(() => {
      minimapFrame = 0
      drawEditorMinimap()
    })
  }

  function drawEditorMinimap() {
    if (!view || !minimapCanvas || !$editorPreferences.editorMinimap) return
    const rect = minimapCanvas.getBoundingClientRect()
    if (rect.width <= 0 || rect.height <= 0) return
    const dpr = window.devicePixelRatio || 1
    minimapCanvas.width = Math.max(1, Math.floor(rect.width * dpr))
    minimapCanvas.height = Math.max(1, Math.floor(rect.height * dpr))
    const ctx = minimapCanvas.getContext('2d')
    if (!ctx) return
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, rect.width, rect.height)

    const styles = getComputedStyle(minimapCanvas)
    const accent = styles.getPropertyValue('--primary').trim() || '#c28a64'
    const muted = styles.getPropertyValue('--muted-foreground').trim() || '#8f8984'
    const lines = view.state.doc.lines
    const lineHeight = Math.max(1, rect.height / Math.max(lines, 1))
    ctx.fillStyle = muted
    ctx.globalAlpha = 0.35
    const stride = Math.max(1, Math.ceil(lines / 900))
    for (let lineNo = 1; lineNo <= lines; lineNo += stride) {
      const text = view.state.doc.line(lineNo).text.trimEnd()
      if (!text) continue
      const y = (lineNo - 1) * lineHeight
      const width = Math.min(rect.width - 8, 8 + text.length * 0.42)
      ctx.fillRect(4, y, Math.max(2, width), Math.max(1, lineHeight * stride * 0.6))
    }
    const scroll = view.scrollDOM
    const maxScroll = Math.max(scroll.scrollHeight, 1)
    const viewportTop = (scroll.scrollTop / maxScroll) * rect.height
    const viewportHeight = Math.max(18, (scroll.clientHeight / maxScroll) * rect.height)
    ctx.globalAlpha = 1
    ctx.fillStyle = colorMix(accent, 0.28)
    ctx.fillRect(1, viewportTop, rect.width - 2, viewportHeight)
    ctx.strokeStyle = accent
    ctx.strokeRect(1.5, viewportTop + 0.5, rect.width - 3, viewportHeight - 1)
  }

  function colorMix(color: string, alpha: number): string {
    if (/^#([0-9a-f]{6})$/i.test(color)) {
      const hex = color.slice(1)
      const r = parseInt(hex.slice(0, 2), 16)
      const g = parseInt(hex.slice(2, 4), 16)
      const b = parseInt(hex.slice(4, 6), 16)
      return `rgba(${r}, ${g}, ${b}, ${alpha})`
    }
    return color
  }

  function scrollFromMinimap(e: PointerEvent) {
    if (!view || !minimapCanvas) return
    const rect = minimapCanvas.getBoundingClientRect()
    const ratio = Math.min(1, Math.max(0, (e.clientY - rect.top) / Math.max(rect.height, 1)))
    const scroll = view.scrollDOM
    scroll.scrollTop = ratio * Math.max(0, scroll.scrollHeight - scroll.clientHeight)
    scheduleMinimapDraw()
  }

  function startMinimapDrag(e: PointerEvent) {
    minimapDragging = true
    minimapCanvas?.setPointerCapture(e.pointerId)
    scrollFromMinimap(e)
  }

  function dragMinimap(e: PointerEvent) {
    if (minimapDragging) scrollFromMinimap(e)
  }

  function stopMinimapDrag(e: PointerEvent) {
    minimapDragging = false
    minimapCanvas?.releasePointerCapture(e.pointerId)
  }

  function attachStickyScroll() {
    scrollDom = view?.scrollDOM ?? null
    scrollDom?.addEventListener('scroll', onEditorScroll, { passive: true })
    updateStickyScope()
    scheduleMinimapDraw()
  }

  function destroyView() {
    if (scrollDom) {
      scrollDom.removeEventListener('scroll', onEditorScroll)
      scrollDom = null
    }
    if (view) {
      view.destroy()
      view = null
    }
    stickyScope = ''
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
  function dirname(path: string): string {
    const idx = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'))
    return idx <= 0 ? path : path.slice(0, idx)
  }

  function sep(path: string): string {
    return path.includes('\\') ? '\\' : '/'
  }

  function isAbsolute(path: string): boolean {
    return /^[a-zA-Z]:[\\/]/.test(path) || path.startsWith('/') || path.startsWith('\\')
  }

  function joinPath(parent: string, child: string): string {
    const s = sep(parent)
    return parent.endsWith(s) ? parent + child : parent + s + child
  }

  function openLinkedPath(raw: string): void {
    const root = get(workspace).folderPath
    const base = mountedPath ? dirname(mountedPath) : root
    const target = isAbsolute(raw) ? raw : base ? joinPath(base, raw) : raw
    void openFile(target, { groupId: groupId ?? undefined })
  }

  function activateThisEditor() {
    if (groupId) setActiveGroup(groupId)
    if (!view) return
    setActiveEditor(view)
    setCursorFromState(view.state)
  }

  /** Tear down the current view and mount the given tab's stored state. */
  function mountTab(id: string) {
    destroyView()
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
    markdownPreviewText = doc
    const onDocChange = () => {
      if (view) recomputeDirty(id, view.state.doc.toString())
      updateStickyScope()
      scheduleMinimapDraw()
      if (/\.md(?:own)?$/i.test(tab.path)) scheduleMarkdownPreview()
      scheduleLspChange()
    }
    // Mirror this tab's cursor into the status bar on every selection change.
    const onSelectionChange = () => {
      if (view) setCursorFromState(view.state)
    }
    const goToDefinition = (line: number, character: number) => {
      void editorGoToDefinitionAt(tab.path, line, character)
    }
    const state = createEditorState(
      doc,
      onDocChange,
      onSelectionChange,
      tab.path,
      openLinkedPath,
      goToDefinition,
    )
    view = mountEditorView(state, host)
    attachStickyScroll()
    mountedId = id
    mountedPath = tab.path
    if (active) {
      setActiveEditor(view)
      // Seed the status bar with the just-mounted tab's cursor position.
      setCursorFromState(view.state)
    }
    // Seed any diagnostics already known for this document.
    applyDiagnosticsToView()
    // Seed the diff-review overlay if this file is part of an active review.
    updateReviewOverlay()
    if (active) view.focus()
  }

  // React to active-tab changes: persist the outgoing tab, mount the incoming.
  // Using $effect so it runs after host is bound and on every activeId change.
  $effect(() => {
    const activeId = tabId ?? $tabs.activeId
    // Check if the tab's path changed even if the id is the same (preview slot
    // replacement: the same slot id gets a new file's content).
    const activeTab = activeId ? $tabs.tabs.find((t) => t.id === activeId) : null
    const activePath = activeTab && isEditorTab(activeTab) ? activeTab.path : null
    if (activeId === mountedId && activePath === mountedPath) return
    const isPreviewSlotReplacement = activeId === mountedId && activePath !== mountedPath
    flushLspChange()
    // Skip persist when the preview slot is being replaced in-place: the slot
    // already holds the new file's fresh EditorState, and calling persistMounted
    // here would overwrite it with the old file's stale view state.
    if (!isPreviewSlotReplacement) persistMounted()
    if (activeId) mountTab(activeId)
    else {
      if (view) {
        destroyView()
      }
      mountedId = null
      mountedPath = null
      setActiveEditor(null)
      clearCursor()
    }
  })

  $effect(() => {
    if (active && view) {
      setActiveEditor(view)
      setCursorFromState(view.state)
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

  $effect(() => {
    void $editorPreferences.editorMinimap
    scheduleMinimapDraw()
  })

  onDestroy(() => {
    flushLspChange()
    persistMounted()
    if (changeTimer) clearTimeout(changeTimer)
    if (markdownTimer) clearTimeout(markdownTimer)
    destroyView()
    if (active) {
      setActiveEditor(null)
      clearCursor()
    }
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
  class="editor-frame"
  class:has-minimap={$editorPreferences.editorMinimap}
  class:markdown-split={$editorPreferences.markdownPreview && !!mountedPath && /\.md(?:own)?$/i.test(mountedPath)}
>
  {#if stickyScope}
    <div class="sticky-scope" title={stickyScope}>{stickyScope}</div>
  {/if}
  <div
    class="editor-host"
    class:inactive={!$appActive}
    bind:this={host}
    onmousedown={activateThisEditor}
    onfocusin={activateThisEditor}
    oncontextmenu={onEditorContextMenu}
  ></div>
  {#if $editorPreferences.editorMinimap}
    <canvas
      bind:this={minimapCanvas}
      class="editor-minimap"
      aria-label="Editor minimap"
      onpointerdown={startMinimapDrag}
      onpointermove={dragMinimap}
      onpointerup={stopMinimapDrag}
      onpointercancel={stopMinimapDrag}
    ></canvas>
  {/if}
  {#if $editorPreferences.markdownPreview && mountedPath && /\.md(?:own)?$/i.test(mountedPath)}
    <MarkdownPreview source={markdownPreviewText} />
  {/if}
</div>

<style>
  .editor-frame {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }
  .editor-host {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    background-color: var(--background);
    color: var(--foreground);
    font-family: var(--font-mono);
    font-size: 13px;
  }
  .editor-frame.has-minimap .editor-host :global(.cm-scroller) {
    margin-right: 82px;
  }
  .editor-frame.markdown-split .editor-host {
    flex-basis: 50%;
  }
  .editor-minimap {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: 78px;
    z-index: 5;
    border-left: 1px solid var(--border);
    background: color-mix(in srgb, var(--background) 88%, var(--card));
    cursor: pointer;
    touch-action: none;
  }
  .sticky-scope {
    position: absolute;
    top: 0;
    left: 42px;
    right: 0;
    z-index: 4;
    height: 24px;
    display: flex;
    align-items: center;
    padding: 0 10px;
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--background) 94%, var(--card));
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    pointer-events: none;
    box-shadow: 0 4px 10px rgba(0, 0, 0, 0.18);
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
  .editor-host :global(.cm-inline-diagnostic) {
    margin: 1px 0 3px 0;
    padding: 2px 10px 2px 14px;
    border-left: 2px solid var(--primary);
    color: var(--muted-foreground);
    background: color-mix(in srgb, var(--background) 82%, var(--card));
    font-family: var(--font-sans);
    font-size: 11px;
    line-height: 1.45;
    white-space: pre-wrap;
  }
  .editor-host :global(.cm-inline-diagnostic-error) {
    border-left-color: var(--destructive);
    color: color-mix(in srgb, var(--destructive) 78%, var(--foreground));
  }
  .editor-host :global(.cm-inline-diagnostic-warning) {
    border-left-color: var(--chart-4, #d29922);
    color: color-mix(in srgb, var(--chart-4, #d29922) 82%, var(--foreground));
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
