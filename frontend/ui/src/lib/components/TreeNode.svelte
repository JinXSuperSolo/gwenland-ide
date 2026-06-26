<script lang="ts">
  import Self from './TreeNode.svelte'
  import { deletePath, listDirectory, moveToTrash, type DirEntry } from '../tauri/commands'
  import { toast } from '../stores/toast'
  import { openFile } from '../stores/tabs'
  import { refreshWorkspace, workspace } from '../stores/workspace'
  import { refreshSignal, collapseSignal, revealSignal, requestTreeRefresh } from '../stores/file-tree'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import { git } from '../stores/git'
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

  async function toggle(permanent = false) {
    if (!entry.is_dir) {
      const res = await openFile(entry.path, { preview: !permanent })
      if (!res.ok && res.error) console.error(res.error)
      return
    }
    expanded = !expanded
    if (expanded && !loaded && !loading) {
      const ok = await loadChildren()
      if (!ok) expanded = false // collapse again so the chevron reflects no children
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

  // GWEN-329: git status color for this node. Files get their own badge letter;
  // folders go amber when any child below them is dirty. Hidden when not a repo.
  const gitClass = $derived.by(() => {
    const state = $git
    if (!state.isRepo) return ''
    const root = $workspace.folderPath
    if (!root) return ''
    const norm = (p: string) => p.replace(/\\/g, '/').replace(/\/+$/, '')
    const rootN = norm(root)
    const selfRel = norm(entry.path).startsWith(rootN + '/')
      ? norm(entry.path).slice(rootN.length + 1)
      : norm(entry.path)
    if (entry.is_dir) {
      // Amber if any changed file lives under this folder.
      const prefix = selfRel + '/'
      return state.files.some((f) => f.path.startsWith(prefix)) ? 'git-dir-dirty' : ''
    }
    const f = state.files.find((x) => x.path === selfRel)
    if (!f) return ''
    switch (f.status) {
      case 'M':
      case 'R':
        return 'git-modified'
      case 'D':
        return 'git-deleted'
      case 'U':
      case 'A':
        return 'git-added'
      default:
        return 'git-modified'
    }
  })

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
  style:padding-left={`${indent}px`}
  role="treeitem"
  aria-selected={false}
  aria-expanded={entry.is_dir ? expanded : undefined}
  tabindex="0"
  onclick={() => toggle(false)}
  ondblclick={(e) => {
    e.preventDefault()
    e.stopPropagation()
    if (!entry.is_dir) void toggle(true)
  }}
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
      toggle(false)
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
