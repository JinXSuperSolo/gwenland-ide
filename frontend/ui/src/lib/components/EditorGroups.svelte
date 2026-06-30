<script lang="ts">
  import { get } from 'svelte/store'
  import {
    tabs,
    closeAllTabs,
    closeSavedTabs,
    closeTab,
    isCommitDiffTab,
    isDiffTab,
    isEditorTab,
    isGitGraphTab,
    isPreviewTab,
    setActiveGroup,
    setGroupSizes,
    showOpenedEditors,
    type EditorGroup,
    type Tab,
  } from '../stores/tabs'
  import { workspace } from '../stores/workspace'
  import {
    editorPreferences,
    toggleMarkdownPreview,
    togglePreviewEditors,
  } from '../stores/editor-preferences'
  import { openSettings } from '../stores/ui'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import { setActiveEditor, editorUndo, editorRedo } from '../editor/active-editor'
  import { clearCursor } from '../stores/cursor'
  import Icon from './Icon.svelte'
  import Tabs from './Tabs.svelte'
  import Editor from './Editor.svelte'
  import EditorBreadcrumbs from './EditorBreadcrumbs.svelte'
  import PreviewPane from './PreviewPane.svelte'
  import GitDiffViewer from './GitDiffViewer.svelte'
  import GitCommitDiffViewer from './git/GitCommitDiffViewer.svelte'
  import GitGraph from './git/GitGraph.svelte'

  let host: HTMLDivElement
  let drag =
    $state<{
      beforeId: string
      afterId: string
      startPos: number
      beforeSize: number
      afterSize: number
      totalPx: number
    } | null>(null)
  let overflowMenuGroupId = $state<string | null>(null)

  const visibleGroups = $derived.by(() => {
    const maximized = $tabs.groups.find((group) => group.isMaximized)
    return maximized ? [maximized] : $tabs.groups
  })

  function onClose(id: string) {
    const tab = get(tabs).tabs.find((candidate) => candidate.id === id)
    if (tab && isEditorTab(tab) && tab.dirty) {
      if (!confirm(`"${tab.name}" has unsaved changes. Close without saving?`)) return
    }
    closeTab(id)
  }

  function onEmptyContextMenu(e: MouseEvent) {
    if (!$workspace.folderPath) return
    openContextMenu(e, { scope: 'workspace_empty', workspaceRoot: $workspace.folderPath })
  }

  function activateGroup(group: EditorGroup) {
    setActiveGroup(group.id)
    const activeTab = group.tabs.find((tab) => tab.id === group.activeId)
    if (!activeTab || !isEditorTab(activeTab)) {
      setActiveEditor(null)
      clearCursor()
    }
  }

  function startResize(e: PointerEvent, before: EditorGroup, after: EditorGroup) {
    if (!host) return
    const rect = host.getBoundingClientRect()
    const totalPx = $tabs.orientation === 'horizontal' ? rect.width : rect.height
    if (totalPx <= 0) return
    drag = {
      beforeId: before.id,
      afterId: after.id,
      startPos: $tabs.orientation === 'horizontal' ? e.clientX : e.clientY,
      beforeSize: before.size,
      afterSize: after.size,
      totalPx,
    }
    e.preventDefault()
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag) return
    const pos = $tabs.orientation === 'horizontal' ? e.clientX : e.clientY
    const total = drag.beforeSize + drag.afterSize
    const delta = ((pos - drag.startPos) / drag.totalPx) * total
    const before = Math.max(0.25, drag.beforeSize + delta)
    const after = Math.max(0.25, drag.afterSize - delta)
    setGroupSizes({ [drag.beforeId]: before, [drag.afterId]: after })
  }

  function stopResize() {
    drag = null
  }

  function runOverflow(action: () => void) {
    overflowMenuGroupId = null
    action()
  }

  function pathForTab(tab: Tab | null): string | null {
    if (!tab) return null
    if (isEditorTab(tab) || isDiffTab(tab)) return tab.path
    if (isPreviewTab(tab) && tab.source.kind === 'static-file') return tab.source.path
    return null
  }

  function isMarkdownTab(tab: Tab | null): boolean {
    return !!tab && isEditorTab(tab) && /\.md(?:own)?$/i.test(tab.path)
  }
</script>

<svelte:window
  onpointermove={onPointerMove}
  onpointerup={stopResize}
  onclick={() => {
    overflowMenuGroupId = null
  }}
/>

<div
  class="editor-groups"
  class:vertical={$tabs.orientation === 'vertical'}
  bind:this={host}
>
  {#each visibleGroups as group, index (group.id)}
    {@const activeTab = group.tabs.find((tab) => tab.id === group.activeId) ?? null}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <section
      class="editor-group"
      class:active={group.id === $tabs.activeGroupId}
      class:locked={group.isLocked}
      style:flex={`${group.size} 1 0`}
      role="group"
      onmousedown={() => activateGroup(group)}
    >
      <div class="group-tabs-row">
        <Tabs groupId={group.id} tabs={group.tabs} activeId={group.activeId} {onClose} />
        <div class="group-toolbar">
          {#if isMarkdownTab(activeTab)}
            <button
              type="button"
              class="toolbar-btn"
              class:active={$editorPreferences.markdownPreview}
              title="Toggle Markdown Preview"
              aria-label="Toggle Markdown Preview"
              aria-pressed={$editorPreferences.markdownPreview}
              onclick={toggleMarkdownPreview}
            ><Icon name="eye" size={14} /></button>
          {/if}
          <button
            type="button"
            class="toolbar-btn"
            title="Undo (Ctrl+Z)"
            aria-label="Undo"
            onclick={() => editorUndo()}
          ><Icon name="undo" size={14} /></button>
          <button
            type="button"
            class="toolbar-btn"
            title="Redo (Ctrl+Y)"
            aria-label="Redo"
            onclick={() => editorRedo()}
          ><Icon name="redo" size={14} /></button>
          <div class="overflow-menu-root">
            <button
              type="button"
              class="toolbar-btn"
              class:active={overflowMenuGroupId === group.id}
              title="More editor actions"
              aria-label="More editor actions"
              aria-haspopup="menu"
              aria-expanded={overflowMenuGroupId === group.id}
              onclick={(e) => {
                e.stopPropagation()
                overflowMenuGroupId = overflowMenuGroupId === group.id ? null : group.id
              }}
            ><Icon name="more-horiz" size={15} /></button>
            {#if overflowMenuGroupId === group.id}
              <div
                class="tabs-overflow-menu gw-anim-slide-down"
                role="menu"
                tabindex="-1"
                onclick={(e) => e.stopPropagation()}
                onkeydown={(e) => e.stopPropagation()}
              >
                <button type="button" role="menuitem" onclick={() => runOverflow(showOpenedEditors)}>
                  <span>Show Opened Editors</span>
                </button>
                <button type="button" role="menuitem" onclick={() => runOverflow(() => closeAllTabs(group.id))}>
                  <span>Close All</span>
                  <kbd>Ctrl+K W</kbd>
                </button>
                <button type="button" role="menuitem" onclick={() => runOverflow(() => closeSavedTabs(group.id))}>
                  <span>Close Saved</span>
                  <kbd>Ctrl+K U</kbd>
                </button>
                <div class="tabs-overflow-separator"></div>
                <button
                  type="button"
                  role="menuitemcheckbox"
                  aria-checked={$editorPreferences.previewEditors}
                  onclick={() => runOverflow(togglePreviewEditors)}
                >
                  <span class="check-slot">
                    {#if $editorPreferences.previewEditors}<Icon name="check" size={13} />{/if}
                  </span>
                  <span>Enable Preview Editors</span>
                </button>
                <button type="button" role="menuitem" onclick={() => runOverflow(openSettings)}>
                  <span>Configure Editors</span>
                </button>
              </div>
            {/if}
          </div>
          {#if group.isLocked || group.isMaximized}
            {#if group.isLocked}<span class="group-flag" title="Group locked">Locked</span>{/if}
            {#if group.isMaximized}<span class="group-flag" title="Group maximized">Max</span>{/if}
          {/if}
        </div>
      </div>

      {#if pathForTab(activeTab)}
        <EditorBreadcrumbs path={pathForTab(activeTab) ?? ''} groupId={group.id} />
      {/if}

      {#if activeTab && isEditorTab(activeTab)}
        <Editor
          tabId={activeTab.id}
          groupId={group.id}
          active={group.id === $tabs.activeGroupId}
        />
      {:else if activeTab && isPreviewTab(activeTab)}
        <PreviewPane source={activeTab.source} />
      {:else if activeTab && isDiffTab(activeTab)}
        <GitDiffViewer root={activeTab.root} path={activeTab.path} untracked={activeTab.untracked} />
      {:else if activeTab && isCommitDiffTab(activeTab)}
        <GitCommitDiffViewer
          workspacePath={activeTab.workspacePath}
          hash={activeTab.hash}
          title={activeTab.message}
        />
      {:else if activeTab && isGitGraphTab(activeTab)}
        <GitGraph workspacePath={activeTab.workspacePath} />
      {:else}
        <div class="empty-state gw-anim-fade" role="presentation" oncontextmenu={onEmptyContextMenu}>
          <div class="empty-brand">GwenLand IDE</div>
          <p class="empty-hint">Open a file from the Explorer to start editing.</p>
        </div>
      {/if}
    </section>

    {#if index < visibleGroups.length - 1}
      <button
        type="button"
        class="group-divider"
        aria-label="Resize editor groups"
        onpointerdown={(e) => startResize(e, group, visibleGroups[index + 1])}
      ></button>
    {/if}
  {/each}
</div>

<style>
  .editor-groups {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    overflow: hidden;
    background-color: var(--background);
  }
  .editor-groups.vertical {
    flex-direction: column;
  }
  .editor-group {
    min-width: 160px;
    min-height: 96px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-right: 1px solid transparent;
    border-bottom: 1px solid transparent;
  }
  .editor-group.active {
    outline: 1px solid color-mix(in srgb, var(--primary) 45%, transparent);
    outline-offset: -1px;
  }
  .group-tabs-row {
    min-height: 36px;
    display: flex;
    align-items: stretch;
    border-bottom: 1px solid var(--border);
    background-color: var(--background);
  }
  .group-tabs-row :global(.tabs-container) {
    flex: 1;
    border-bottom: none;
  }
  .group-toolbar {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 0 4px;
    flex-shrink: 0;
    border-left: 1px solid var(--border);
    position: relative;
  }
  .toolbar-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: color 0.12s ease, background-color 0.12s ease;
  }
  .toolbar-btn:hover {
    color: var(--foreground);
    background-color: var(--secondary);
  }
  .toolbar-btn.active {
    color: var(--foreground);
    background-color: var(--secondary);
  }
  .overflow-menu-root {
    display: flex;
    align-items: center;
  }
  .tabs-overflow-menu {
    position: absolute;
    top: calc(100% - 2px);
    right: 4px;
    z-index: 75;
    min-width: 220px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--popover);
    box-shadow: var(--shadow-lg);
  }
  .tabs-overflow-menu button {
    width: 100%;
    min-height: 28px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 5px 8px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .tabs-overflow-menu button:hover {
    background-color: var(--sidebar-accent);
  }
  .tabs-overflow-menu kbd {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--muted-foreground);
  }
  .check-slot {
    width: 14px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--primary);
  }
  .tabs-overflow-separator {
    height: 1px;
    margin: 4px 6px;
    background-color: var(--border);
  }
  .group-flag {
    font-size: 10px;
    color: var(--muted-foreground);
    padding: 0 4px;
    white-space: nowrap;
  }
  .group-divider {
    flex: 0 0 5px;
    border: none;
    padding: 0;
    background: var(--border);
    cursor: col-resize;
    opacity: 0.7;
  }
  .editor-groups.vertical .group-divider {
    width: 100%;
    height: 5px;
    cursor: row-resize;
  }
  .group-divider:hover {
    background: var(--primary);
  }
  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 24px;
  }
  .empty-brand {
    font-size: 20px;
    font-weight: 700;
    color: var(--primary);
    letter-spacing: var(--tracking-tight);
    opacity: 0.9;
  }
  .empty-hint {
    font-size: 13px;
    color: var(--muted-foreground);
  }
</style>
