<script lang="ts">
  import { onMount } from 'svelte'
  import { collapsePanel } from '../stores/panels'
  import { workspace, openFolder, openFolderPath } from '../stores/workspace'
  import { newUntitledFile } from '../stores/tabs'
  import { getRecentProjects, type RecentProject } from '../tauri/commands'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import TreeNode from './TreeNode.svelte'
  import Icon from './Icon.svelte'

  // Display just the folder's basename in the header when one is open.
  const folderName = $derived.by(() => {
    const p = $workspace.folderPath
    if (!p) return null
    const parts = p.split(/[\\/]/).filter(Boolean)
    return parts.length ? parts[parts.length - 1] : p
  })

  let recents = $state<RecentProject[]>([])

  onMount(async () => {
    try {
      recents = await getRecentProjects()
    } catch {
      recents = []
    }
  })

  function basename(p: string): string {
    return p.split(/[\\/]/).filter(Boolean).pop() || p
  }

  // Right-click on the Explorer's blank area (below/around the tree) opens the
  // workspace context menu. File/folder nodes stop propagation, so this only
  // fires for empty space. No-op until a folder is open.
  function onEmptyContextMenu(e: MouseEvent) {
    if (!$workspace.folderPath) return
    openContextMenu(e, { scope: 'workspace_empty', workspaceRoot: $workspace.folderPath })
  }
</script>

<aside class="file-tree" aria-label="File Tree">
  <header class="panel-header">
    <span class="panel-title">{folderName ?? 'Explorer'}</span>
    <div class="header-actions">
      <button
        class="header-btn"
        title="Open Folder"
        aria-label="Open Folder"
        onclick={openFolder}
      ><Icon name="folder" size={15} /></button>
      <button
        class="header-btn"
        title="Collapse File Tree"
        aria-label="Collapse File Tree"
        onclick={() => collapsePanel('fileTree')}
      ><Icon name="xmark" size={16} /></button>
    </div>
  </header>

  <div class="panel-body" oncontextmenu={onEmptyContextMenu} role="presentation">
    {#if !$workspace.folderPath}
      <div class="empty gw-anim-fade">
        <button type="button" class="action-btn gw-transition" onclick={() => newUntitledFile()}>
          <Icon name="page-plus" class="ab-icon" />Create File
        </button>
        <button type="button" class="action-btn gw-transition" onclick={() => void openFolder()}>
          <Icon name="folder" class="ab-icon" />Open Folder
        </button>

        <div class="recent-block">
          <div class="recent-title">Open Recent</div>
          {#if recents.length === 0}
            <div class="recent-empty">No recent folders</div>
          {:else}
            {#each recents.slice(0, 8) as r (r.path)}
              <button
                type="button"
                class="recent-item gw-transition"
                title={r.path}
                onclick={() => void openFolderPath(r.path)}
              >
                <Icon name="clock-rotate-right" size={14} class="ri-icon" />
                <span class="ri-name">{basename(r.path)}</span>
              </button>
            {/each}
          {/if}
        </div>
      </div>
    {:else if $workspace.loading}
      <p class="placeholder">Loading…</p>
    {:else if $workspace.error}
      <p class="placeholder error">{$workspace.error}</p>
    {:else}
      <div role="tree" class="tree">
        {#each $workspace.rootEntries as entry (entry.path)}
          <TreeNode {entry} depth={0} />
        {/each}
      </div>
    {/if}
  </div>
</aside>

<style>
  .file-tree {
    height: 100%;
    background-color: var(--sidebar);
    border-right: 1px solid var(--sidebar-border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .panel-header {
    height: 38px;
    flex-shrink: 0;
    padding: 0 6px 0 14px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--sidebar-border);
  }
  .panel-title {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: var(--tracking-wider);
    color: var(--muted-foreground);
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .header-actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }
  .header-btn {
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    font-size: 15px;
    line-height: 1;
    width: 22px;
    height: 22px;
    border-radius: var(--radius-sm);
  }
  .header-btn:hover {
    color: var(--foreground);
    background-color: var(--sidebar-accent);
  }
  .panel-body {
    flex: 1;
    overflow: auto;
    padding: 4px 0;
  }
  .tree {
    display: flex;
    flex-direction: column;
  }
  .empty {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 12px;
  }
  .placeholder {
    font-size: 12px;
    color: var(--muted-foreground);
    padding: 6px 14px;
  }
  .placeholder.error {
    color: var(--destructive);
  }
  .action-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    background-color: var(--secondary);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-family: var(--font-sans);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
  }
  .action-btn:hover {
    background-color: var(--sidebar-accent);
    border-color: var(--primary);
  }
  .action-btn :global(.ab-icon) {
    color: var(--primary);
  }
  .recent-block {
    margin-top: 14px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .recent-title {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: var(--tracking-wider);
    color: var(--muted-foreground);
    padding: 0 4px 6px;
  }
  .recent-empty {
    font-size: 12px;
    color: var(--muted-foreground);
    opacity: 0.7;
    padding: 2px 4px;
  }
  .recent-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--sidebar-foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
  }
  .recent-item :global(.ri-icon) {
    color: var(--muted-foreground);
  }
  .ri-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .recent-item:hover {
    background-color: var(--sidebar-accent);
    color: var(--primary);
  }
  .recent-item:hover :global(.ri-icon) {
    color: var(--primary);
  }
</style>
