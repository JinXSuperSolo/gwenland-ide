<script lang="ts">
  import { fsWatchDir, fsUnwatchDir, type FlatRow } from '../tauri/commands'
  import { toast } from '../stores/toast'
  import { openFile } from '../stores/tabs'
  import { workspace } from '../stores/workspace'
  import { toggleRow } from '../stores/tree'
  import { optimisticDeletePath, optimisticPermanentDeletePath } from '../stores/file-ops'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import { git, gitDirtyPrefixes } from '../stores/git'
  import { perfSettings } from '../stores/performance'
  import Icon from './Icon.svelte'
  import FileIcon from './FileIcon.svelte'

  /**
   * A single flat row in the virtualized file tree (M19 Wave 2). All shape data
   * (depth, expanded, has_children) comes from the Rust-owned tree; this
   * component is pure presentation + event wiring. No recursion — siblings and
   * children are separate rows in the parent array.
   */
  let { row }: { row: FlatRow } = $props()

  // M19 Wave 1/2: a folder is watched for FS changes only while expanded. The
  // tree store owns expand/collapse; we mirror the watch registration off the
  // row's `is_expanded` flag so it stays in sync through any expand path.
  let watched = false
  $effect(() => {
    const shouldWatch = row.is_dir && row.is_expanded
    if (shouldWatch && !watched) {
      watched = true
      void fsWatchDir(row.path)
    } else if (!shouldWatch && watched) {
      watched = false
      void fsUnwatchDir(row.path)
    }
  })

  async function activate() {
    if (row.is_dir) {
      await toggleRow(row)
      return
    }
    const res = await openFile(row.path)
    if (!res.ok && res.error) console.error(res.error)
  }

  async function moveEntryToTrash() {
    const root = $workspace.folderPath
    if (!root) return
    if (!confirm(`Move "${row.name}" to Trash?`)) return
    await new Promise<void>((resolve) => setTimeout(resolve, 0))
    if (await optimisticDeletePath(row.path, root)) {
      toast(`"${row.name}" moved to Trash`, 'success')
    }
  }

  async function deleteEntryPermanently() {
    const root = $workspace.folderPath
    if (!root) return
    if (!confirm(`Are you sure? This cannot be undone.\n\nDelete "${row.name}" permanently?`)) return
    await new Promise<void>((resolve) => setTimeout(resolve, 0))
    if (await optimisticPermanentDeletePath(row.path, root)) {
      toast(`"${row.name}" deleted permanently`, 'success')
    }
  }

  // Indent each level; base padding keeps the first level off the edge.
  const indent = $derived(8 + row.depth * 14)

  // GWEN-329: git status color + badge letter. Files: O(1) map lookup.
  // Folders: O(1) set lookup via precomputed dirty-prefix set.
  const gitInfo = $derived.by(() => {
    const state = $git
    if (!state.isRepo) return { cls: '', badge: '' }
    const root = $workspace.folderPath
    if (!root) return { cls: '', badge: '' }
    const norm = (p: string) => p.replace(/\\/g, '/').replace(/\/+$/, '')
    const rootN = norm(root)
    const selfRel = norm(row.path).startsWith(rootN + '/')
      ? norm(row.path).slice(rootN.length + 1)
      : norm(row.path)
    if (row.is_dir) {
      const dirty = $gitDirtyPrefixes.has(selfRel + '/')
      return { cls: dirty ? 'git-dir-dirty' : '', badge: '' }
    }
    const f = state.files.find((x) => x.path === selfRel)
    if (!f) return { cls: '', badge: '' }
    switch (f.status) {
      case 'M':
      case 'R':
        return { cls: 'git-modified', badge: 'M' }
      case 'D':
        return { cls: 'git-deleted', badge: 'D' }
      case 'U':
        return { cls: 'git-added', badge: 'U' }
      case 'A':
        return { cls: 'git-added', badge: 'A' }
      default:
        return { cls: 'git-modified', badge: 'M' }
    }
  })
  // M19 Wave 5: Low-End Mode suppresses git decorations on tree rows.
  const gitClass = $derived($perfSettings.showGitBadges ? gitInfo.cls : '')
  const gitBadge = $derived($perfSettings.showGitBadges ? gitInfo.badge : '')

  function onContextMenu(e: MouseEvent) {
    openContextMenu(e, {
      scope: 'file_tree',
      path: row.path,
      isDirectory: row.is_dir,
      workspaceRoot: $workspace.folderPath ?? undefined,
    })
  }
</script>

<div
  class="node-row"
  class:is-dir={row.is_dir}
  class:no-guides={!$perfSettings.showIndentGuides}
  style={`padding-left: ${indent}px; --tree-depth: ${row.depth};`}
  role="treeitem"
  aria-selected={false}
  aria-expanded={row.is_dir ? row.is_expanded : undefined}
  tabindex="0"
  onclick={() => void activate()}
  ondblclick={(e) => { e.preventDefault(); e.stopPropagation() }}
  oncontextmenu={onContextMenu}
  onkeydown={(e) => {
    if (e.key === 'Delete') {
      e.preventDefault()
      e.stopPropagation()
      if (e.shiftKey) void deleteEntryPermanently()
      else void moveEntryToTrash()
      return
    }
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      void activate()
    }
  }}
>
  {#if row.is_dir}
    <span class="chevron" class:open={row.is_expanded} class:hidden={!row.has_children}>
      <Icon name="nav-arrow-right" size={14} />
    </span>
    {#if $perfSettings.showFileIcons}<FileIcon dir open={row.is_expanded} size={16} />{/if}
  {:else}
    <span class="chevron spacer"></span>
    {#if $perfSettings.showFileIcons}<FileIcon name={row.name} size={16} />{/if}
  {/if}
  <span class={`node-name ${gitClass}`}>{row.name}</span>
  {#if gitBadge}
    <span class={`git-badge ${gitClass}`}>{gitBadge}</span>
  {/if}
</div>

<style>
  .node-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding-right: 8px;
    font-size: 13px;
    color: var(--sidebar-foreground);
    cursor: pointer;
    white-space: nowrap;
    user-select: none;
  }
  /* Low-End Mode hides the indent guide gradient (one less paint per row). */
  .node-row.no-guides::before {
    display: none;
  }
  .node-row::before {
    content: '';
    position: absolute;
    inset: 0 auto 0 8px;
    width: calc(var(--tree-depth) * 14px);
    pointer-events: none;
    background-image: repeating-linear-gradient(
      to right,
      transparent 0,
      transparent 13px,
      color-mix(in srgb, var(--sidebar-border) 82%, transparent) 13px,
      color-mix(in srgb, var(--sidebar-border) 82%, transparent) 14px
    );
    opacity: 0.8;
  }
  .node-row:hover {
    background-color: var(--sidebar-accent);
  }
  .node-row:focus-visible {
    outline: 1px solid rgba(var(--ring-rgb), 0.6);
    outline-offset: -1px;
  }
  .chevron {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 12px;
    flex-shrink: 0;
    font-size: 9px;
    color: var(--muted-foreground);
    transition: transform 0.12s ease;
  }
  .chevron.open {
    transform: rotate(90deg);
  }
  .chevron.spacer,
  .chevron.hidden {
    visibility: hidden;
  }
  .node-name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .is-dir .node-name {
    font-weight: 500;
  }
  /* GWEN-329: git status colors (VS Code palette). */
  .node-name.git-modified,
  .node-name.git-dir-dirty {
    color: #e2c08d;
  }
  .node-name.git-added {
    color: #89d185;
  }
  .node-name.git-deleted {
    color: #f14c4c;
    text-decoration: line-through;
  }
  .git-badge {
    flex-shrink: 0;
    margin-left: auto;
    padding: 0 3px;
    font-size: 10px;
    font-weight: 700;
    line-height: 14px;
    border-radius: 3px;
    font-family: var(--font-mono);
    letter-spacing: 0;
  }
  .git-badge.git-modified {
    color: #e5c07b;
    background: color-mix(in srgb, #e5c07b 14%, transparent);
  }
  .git-badge.git-added {
    color: #98c379;
    background: color-mix(in srgb, #98c379 14%, transparent);
  }
  .git-badge.git-deleted {
    color: #e06c75;
    background: color-mix(in srgb, #e06c75 14%, transparent);
  }
</style>
