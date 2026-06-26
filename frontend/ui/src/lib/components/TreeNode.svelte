<script lang="ts">
  import Self from './TreeNode.svelte'
  import { deletePath, listDirectory, moveToTrash, type DirEntry } from '../tauri/commands'
  import { toast } from '../stores/toast'
  import { openFile } from '../stores/tabs'
  import { refreshWorkspace, workspace } from '../stores/workspace'
  import { refreshSignal, collapseSignal, revealSignal, requestTreeRefresh } from '../stores/file-tree'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import { git, gitDirtyPrefixes } from '../stores/git'
  import Icon from './Icon.svelte'
  import FileIcon from './FileIcon.svelte'

  /**
   * A single row in the file tree. Folders are expandable: their children are
   * fetched lazily on first expand via listDirectory (backend-sorted — we do
   * NOT re-sort here). Files emit their path on click (Wave 2 stops at logging;
   * opening a tab is Wave 3).
   */
  let { entry, depth = 0 }: { entry: DirEntry; depth?: number } = $props()

  let expanded = $state(false)
  let loaded = $state(false)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let children = $state<DirEntry[]>([])

  async function toggle() {
    if (!entry.is_dir) {
      const res = await openFile(entry.path)
      if (!res.ok && res.error) console.error(res.error)
      return
    }
    expanded = !expanded
    if (expanded && !loaded && !loading) {
      const ok = await loadChildren()
      if (!ok) expanded = false
    }
  }

  /** Fetch (or re-fetch) this folder's children. Returns false on error. */
  async function loadChildren(): Promise<boolean> {
    loading = true
    error = null
    try {
      children = await listDirectory(entry.path)
      loaded = true
      return true
    } catch (e) {
      error = String(e)
      return false
    } finally {
      loading = false
    }
  }

  // React to M9 tree signals. Each fires once per nonce; the effect depends only
  // on the signal store, so unrelated state changes don't re-trigger it.
  let seenRefresh = 0
  $effect(() => {
    const sig = $refreshSignal
    if (!sig || sig.nonce === seenRefresh) return
    seenRefresh = sig.nonce
    if (sig.path === entry.path && entry.is_dir && loaded) void loadChildren()
  })

  let seenCollapse = 0
  $effect(() => {
    const sig = $collapseSignal
    if (!sig || sig.nonce === seenCollapse) return
    seenCollapse = sig.nonce
    if (sig.path === entry.path && entry.is_dir) expanded = false
  })

  // True when `target` lives somewhere under `dir` (separator-normalized).
  function isInside(target: string, dir: string): boolean {
    const norm = (p: string) => p.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()
    return norm(target).startsWith(norm(dir) + '/')
  }

  function dirname(path: string): string {
    const idx = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'))
    return idx <= 0 ? path : path.slice(0, idx)
  }

  function samePath(a: string, b: string): boolean {
    const norm = (p: string) => p.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase()
    return norm(a) === norm(b)
  }

  function refreshParent(root: string) {
    const parent = dirname(entry.path)
    if (samePath(parent, root)) void refreshWorkspace()
    else requestTreeRefresh(parent)
  }

  async function moveEntryToTrash() {
    const root = $workspace.folderPath
    if (!root) return
    if (!confirm(`Move "${entry.name}" to Trash?`)) return
    await new Promise<void>((resolve) => setTimeout(resolve, 0))
    try {
      await moveToTrash(entry.path, root)
      refreshParent(root)
      toast(`"${entry.name}" moved to Trash`, 'success')
    } catch (e) {
      alert(`Could not move to trash: ${e}`)
    }
  }

  async function deleteEntryPermanently() {
    const root = $workspace.folderPath
    if (!root) return
    if (!confirm(`Are you sure? This cannot be undone.\n\nDelete "${entry.name}" permanently?`)) {
      return
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 0))
    try {
      await deletePath(entry.path, root)
      refreshParent(root)
      toast(`"${entry.name}" deleted permanently`, 'success')
    } catch (e) {
      alert(`Could not delete permanently: ${e}`)
    }
  }

  // Reveal: a folder that contains the target expands toward it. Children mount
  // after loading and re-check the still-current signal, so the expansion
  // cascades down to the file's folder.
  let seenReveal = 0
  $effect(() => {
    const sig = $revealSignal
    if (!sig || sig.nonce === seenReveal) return
    seenReveal = sig.nonce
    if (entry.is_dir && sig.path !== entry.path && isInside(sig.path, entry.path) && !expanded) {
      expanded = true
      if (!loaded && !loading) void loadChildren()
    }
  })

  // Indent each level; base padding keeps the first level off the edge.
  const indent = $derived(8 + depth * 14)

  // GWEN-329: git status color + badge letter for this node.
  // Files: O(1) map lookup. Folders: O(1) set lookup via precomputed prefix set.
  const gitInfo = $derived.by(() => {
    const state = $git
    if (!state.isRepo) return { cls: '', badge: '' }
    const root = $workspace.folderPath
    if (!root) return { cls: '', badge: '' }
    const norm = (p: string) => p.replace(/\\/g, '/').replace(/\/+$/, '')
    const rootN = norm(root)
    const selfRel = norm(entry.path).startsWith(rootN + '/')
      ? norm(entry.path).slice(rootN.length + 1)
      : norm(entry.path)
    if (entry.is_dir) {
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
  const gitClass = $derived(gitInfo.cls)
  const gitBadge = $derived(gitInfo.badge)

  // Right-click opens the registry-driven context menu (M9). It only sends the
  // node's context; the registry decides which actions apply.
  function onContextMenu(e: MouseEvent) {
    openContextMenu(e, {
      scope: 'file_tree',
      path: entry.path,
      isDirectory: entry.is_dir,
      workspaceRoot: $workspace.folderPath ?? undefined,
    })
  }
</script>

<div
  class="node-row gw-anim-fade"
  class:is-dir={entry.is_dir}
  style={`padding-left: ${indent}px; --tree-depth: ${depth};`}
  role="treeitem"
  aria-selected={false}
  aria-expanded={entry.is_dir ? expanded : undefined}
  tabindex="0"
  onclick={() => void toggle()}
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
      void toggle()
    }
  }}
>
  {#if entry.is_dir}
    <span class="chevron" class:open={expanded}>
      <Icon name="nav-arrow-right" size={14} />
    </span>
    <FileIcon dir open={expanded} size={16} />
  {:else}
    <span class="chevron spacer"></span>
    <FileIcon name={entry.name} size={16} />
  {/if}
  <span class={`node-name ${gitClass}`}>{entry.name}</span>
  {#if gitBadge}
    <span class={`git-badge ${gitClass}`}>{gitBadge}</span>
  {/if}
</div>

{#if entry.is_dir && expanded}
  {#if loading}
    <div class="node-info" style:padding-left={`${indent + 14}px`}>Loading…</div>
  {:else if error}
    <div class="node-info node-error" style:padding-left={`${indent + 14}px`}>
      {error}
    </div>
  {:else}
    {#each children as child (child.path)}
      <Self entry={child} depth={depth + 1} />
    {/each}
  {/if}
{/if}

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
  .chevron.spacer {
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
  /* Git status letter badge — flush right after the filename. */
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
  .node-info {
    height: 22px;
    display: flex;
    align-items: center;
    font-size: 12px;
    color: var(--muted-foreground);
  }
  .node-error {
    color: var(--destructive);
  }
</style>
