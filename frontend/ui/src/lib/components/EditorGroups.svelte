<script lang="ts">
  import { get } from 'svelte/store'
  import {
    tabs,
    closeTab,
    isDiffTab,
    isEditorTab,
    isPreviewTab,
    setActiveGroup,
    setGroupSizes,
    type EditorGroup,
  } from '../stores/tabs'
  import { workspace } from '../stores/workspace'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import { setActiveEditor, editorUndo, editorRedo } from '../editor/active-editor'
  import { clearCursor } from '../stores/cursor'
  import Icon from './Icon.svelte'
  import Tabs from './Tabs.svelte'
  import Editor from './Editor.svelte'
  import PreviewPane from './PreviewPane.svelte'
  import GitDiffViewer from './GitDiffViewer.svelte'

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
</script>

<svelte:window onpointermove={onPointerMove} onpointerup={stopResize} />

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
          {#if group.isLocked || group.isMaximized}
            {#if group.isLocked}<span class="group-flag" title="Group locked">Locked</span>{/if}
            {#if group.isMaximized}<span class="group-flag" title="Group maximized">Max</span>{/if}
          {/if}
        </div>
      </div>

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
  .group-tabs-row :global(.tabs-bar) {
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
