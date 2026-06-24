<script lang="ts">
  import { get } from 'svelte/store'
  import { tabs, closeTab, isEditorTab, isPreviewTab, isDiffTab } from '../stores/tabs'
  import { workspace } from '../stores/workspace'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import Tabs from './Tabs.svelte'
  import Editor from './Editor.svelte'
  import PreviewPane from './PreviewPane.svelte'
  import GitDiffViewer from './GitDiffViewer.svelte'

  // The active tab drives which surface fills the workspace below the tab strip.
  const activeTab = $derived($tabs.tabs.find((t) => t.id === $tabs.activeId) ?? null)

  // Close from the tab's × button: prompt on unsaved changes before discarding.
  function onClose(id: string) {
    const tab = get(tabs).tabs.find((t) => t.id === id)
    if (tab && isEditorTab(tab) && tab.dirty) {
      const ok = confirm(`"${tab.name}" has unsaved changes. Close without saving?`)
      if (!ok) return
    }
    closeTab(id)
  }

  // Right-click the empty workspace area → workspace context menu (New File,
  // etc.). No-op until a folder is open. The editor surface has its own menu.
  function onEmptyContextMenu(e: MouseEvent) {
    if (!$workspace.folderPath) return
    openContextMenu(e, { scope: 'workspace_empty', workspaceRoot: $workspace.folderPath })
  }
</script>

<main class="workspace" aria-label="Workspace">
  <Tabs {onClose} />

  {#if activeTab && isEditorTab(activeTab)}
    <Editor />
  {:else if activeTab && isPreviewTab(activeTab)}
    <PreviewPane source={activeTab.source} />
  {:else if activeTab && isDiffTab(activeTab)}
    <GitDiffViewer root={activeTab.root} path={activeTab.path} untracked={activeTab.untracked} />
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="empty-state gw-anim-fade" oncontextmenu={onEmptyContextMenu}>
      <div class="empty-brand">GwenLand IDE</div>
      <p class="empty-hint">Open a file from the Explorer to start editing.</p>
    </div>
  {/if}
</main>

<style>
  .workspace {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background-color: var(--background);
  }
  /* Simple centered empty state when no tab is open. */
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
