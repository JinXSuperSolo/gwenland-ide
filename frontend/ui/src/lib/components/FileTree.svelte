<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { collapsePanel } from '../stores/panels'
  import { workspace, openFolder, openFolderPath } from '../stores/workspace'
  import { newUntitledFile } from '../stores/tabs'
  import { getRecentProjects, type RecentProject } from '../tauri/commands'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import { requestTreeCollapse } from '../stores/file-tree'
  import { refreshWorkspace } from '../stores/workspace'
  import { treeInput, cancelTreeInput, confirmTreeInput } from '../stores/tree-input'
  import { openFile } from '../stores/tabs'
  import { treeRows } from '../stores/tree'
  import { optimisticCreateDir, optimisticCreateFile, undoLastFileOp } from '../stores/file-ops'
  import FileTreeRow from './FileTreeRow.svelte'
  import Icon from './Icon.svelte'

  // --- Virtual scroll (M19 Wave 2, scratch) --------------------------------
  // Only visible rows + overscan are rendered, so a 10k-file workspace stays
  // smooth. The spacer reserves full scroll height; rows are absolutely shifted
  // by `offsetY` so the scrollbar reflects the whole list.
  const ROW_HEIGHT = 24 // px — matches .node-row height in FileTreeRow
  const OVERSCAN = 20 // rows rendered above/below the viewport

  let viewport = $state<HTMLDivElement | null>(null)
  let scrollTop = $state(0)
  let viewportHeight = $state(0)

  const totalHeight = $derived($treeRows.length * ROW_HEIGHT)
  const visibleStart = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN))
  const visibleEnd = $derived(
    Math.min(
      $treeRows.length,
      visibleStart + Math.ceil(viewportHeight / ROW_HEIGHT) + OVERSCAN * 2,
    ),
  )
  const visibleRows = $derived($treeRows.slice(visibleStart, visibleEnd))
  const offsetY = $derived(visibleStart * ROW_HEIGHT)

  function onTreeScroll(e: Event) {
    scrollTop = (e.currentTarget as HTMLDivElement).scrollTop
  }

  // Track the viewport height so the visible window resizes with the panel.
  $effect(() => {
    if (!viewport) return
    const ro = new ResizeObserver((entries) => {
      viewportHeight = entries[0].contentRect.height
    })
    ro.observe(viewport)
    viewportHeight = viewport.clientHeight
    return () => ro.disconnect()
  })

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
  function sep(p: string): string {
    return p.includes('\\') ? '\\' : '/'
  }
  function join(parent: string, name: string): string {
    const s = sep(parent)
    return parent.endsWith(s) ? parent + name : parent + s + name
  }

  // Right-click on the Explorer's blank area opens the workspace context menu.
  function onEmptyContextMenu(e: MouseEvent) {
    if (!$workspace.folderPath) return
    openContextMenu(e, { scope: 'workspace_empty', workspaceRoot: $workspace.folderPath })
  }

  // Header button actions — trigger tree-input for new file / new folder.
  function triggerNewFile() {
    const root = $workspace.folderPath
    if (!root) return
    import('../stores/tree-input').then(({ openTreeInput }) => {
      void openTreeInput({ kind: 'file', targetDir: root, icon: 'page' }).then(async (name) => {
        if (!name) return
        const target = join(root, name)
        if (await optimisticCreateFile(target, root)) await openFile(target)
      })
    })
  }

  function triggerNewFolder() {
    const root = $workspace.folderPath
    if (!root) return
    import('../stores/tree-input').then(({ openTreeInput }) => {
      void openTreeInput({ kind: 'folder', targetDir: root, icon: 'folder' }).then(async (name) => {
        if (!name) return
        await optimisticCreateDir(join(root, name), root)
      })
    })
  }

  function collapseAll() {
    const root = $workspace.folderPath
    if (!root) return
    requestTreeCollapse(root)
  }

  function refreshAll() {
    void refreshWorkspace()
  }

  // Inline input row logic.
  let inputEl = $state<HTMLInputElement | null>(null)
  let inputValue = $state('')

  // When the store opens, seed the input value and focus.
  $effect(() => {
    if ($treeInput.open) {
      inputValue = $treeInput.initialValue
      tick().then(() => {
        if (inputEl) {
          inputEl.focus()
          // For rename: select the name without extension so user can type over it.
          if ($treeInput.kind === 'rename') {
            const dot = inputValue.lastIndexOf('.')
            if (dot > 0) inputEl.setSelectionRange(0, dot)
            else inputEl.select()
          } else {
            inputEl.select()
          }
        }
      })
    }
  })

  function onInputKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      confirmTreeInput(inputValue)
    } else if (e.key === 'Escape') {
      e.preventDefault()
      cancelTreeInput()
    }
  }

  function onInputBlur() {
    // Blur without Enter = cancel (same as VS Code).
    cancelTreeInput()
  }

  function onTreeKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === 'z') {
      e.preventDefault()
      e.stopPropagation()
      void undoLastFileOp()
    }
  }
</script>

<aside class="file-tree" aria-label="File Tree">
  <header class="panel-header">
    <span class="panel-title">{folderName ?? 'Explorer'}</span>
    <div class="header-actions">
      {#if $workspace.folderPath}
        <button
          class="header-btn"
          title="New File"
          aria-label="New File"
          onclick={triggerNewFile}
        ><Icon name="page-plus" size={15} /></button>
        <button
          class="header-btn"
          title="New Folder"
          aria-label="New Folder"
          onclick={triggerNewFolder}
        ><Icon name="folder-plus" size={15} /></button>
        <button
          class="header-btn"
          title="Collapse All"
          aria-label="Collapse All"
          onclick={collapseAll}
        ><Icon name="collapse" size={15} /></button>
        <button
          class="header-btn"
          title="Refresh"
          aria-label="Refresh"
          onclick={refreshAll}
        ><Icon name="refresh" size={15} /></button>
      {:else}
        <button
          class="header-btn"
          title="Open Folder"
          aria-label="Open Folder"
          onclick={openFolder}
        ><Icon name="folder" size={15} /></button>
      {/if}
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
      <!-- Inline input row — appears at the top of the tree when active. -->
      {#if $treeInput.open}
        <div class="inline-input-row">
          <Icon name={$treeInput.icon as 'page' | 'folder'} size={16} class="ii-icon" />
          <input
            bind:this={inputEl}
            bind:value={inputValue}
            class="inline-input"
            type="text"
            spellcheck="false"
            onkeydown={onInputKeydown}
            onblur={onInputBlur}
          />
        </div>
      {/if}
      <div
        class="tree-viewport"
        bind:this={viewport}
        onscroll={onTreeScroll}
        onkeydown={onTreeKeydown}
        role="tree"
        tabindex="-1"
      >
        <div class="tree-spacer" style={`height: ${totalHeight}px`}>
          <div class="tree-rows" style={`transform: translateY(${offsetY}px)`}>
            {#each visibleRows as row (row.id)}
              <FileTreeRow {row} />
            {/each}
          </div>
        </div>
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
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .header-btn:hover {
    color: var(--foreground);
    background-color: var(--sidebar-accent);
  }
  .panel-body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    padding: 4px 0;
    display: flex;
    flex-direction: column;
  }
  /* Empty state / placeholders need their own scroll since panel-body no longer
     scrolls (the tree viewport owns scrolling when a folder is open). */
  .panel-body > .empty,
  .panel-body > .placeholder {
    overflow: auto;
  }
  /* Virtual-scroll viewport (M19 Wave 2). */
  .tree-viewport {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    position: relative;
  }
  .tree-spacer {
    position: relative;
    width: 100%;
  }
  .tree-rows {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    flex-direction: column;
    will-change: transform;
  }

  /* Inline input row — mimics a tree node row. */
  .inline-input-row {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding: 0 8px 0 22px;
  }
  .inline-input-row :global(.ii-icon) {
    flex-shrink: 0;
    color: var(--muted-foreground);
  }
  .inline-input {
    flex: 1;
    min-width: 0;
    height: 18px;
    background: var(--input, var(--secondary));
    border: 1px solid var(--primary);
    border-radius: 2px;
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    padding: 0 4px;
    outline: none;
    box-shadow: 0 0 0 1px var(--primary);
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
