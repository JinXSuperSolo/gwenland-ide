<script lang="ts">
  import { git, refreshGit } from '../stores/git'
  import { workspace } from '../stores/workspace'
  import { openPrompt } from '../stores/prompt-dialog'
  import {
    gitListBranches,
    gitCheckout,
    gitCreateBranch,
  } from '../tauri/commands'
  import Icon from './Icon.svelte'

  // GWEN-331: branch picker shown above the status-bar branch button. Lists
  // local branches, highlights the current one, and offers "create new". Switch
  // warns (non-blocking) if there are uncommitted changes.
  let {
    open = false,
    anchorEl,
    onClose,
  }: { open?: boolean; anchorEl?: HTMLElement; onClose: () => void } = $props()

  let branches = $state<string[]>([])
  let loading = $state(false)
  let busyMsg = $state<string | null>(null)

  const root = $derived($workspace.folderPath)

  // Load the branch list whenever the picker opens.
  $effect(() => {
    if (open && root) void load()
  })

  async function load() {
    loading = true
    busyMsg = null
    try {
      branches = await gitListBranches(root!)
    } catch (e) {
      busyMsg = String(e)
    } finally {
      loading = false
    }
  }

  async function switchTo(branch: string) {
    if (!root || branch === $git.branch) {
      onClose()
      return
    }
    // Non-blocking warning if the worktree is dirty (GWEN-331).
    if ($git.dirtyCount > 0) {
      const ok = confirm(
        `You have ${$git.dirtyCount} uncommitted change(s). Switching branches may carry them over or conflict. Continue?`
      )
      if (!ok) return
    }
    busyMsg = 'Switching…'
    try {
      await gitCheckout(root, branch)
      await refreshGit()
      onClose()
    } catch (e) {
      busyMsg = String(e)
    }
  }

  async function createNew() {
    if (!root) return
    const name = await openPrompt({
      title: 'Create Branch',
      label: 'New branch name',
      placeholder: 'my-feature',
    })
    if (!name) return
    busyMsg = 'Creating…'
    try {
      await gitCreateBranch(root, name)
      await refreshGit()
      onClose()
    } catch (e) {
      busyMsg = String(e)
    }
  }

  // Position the popover just above the anchor button.
  let menuEl = $state<HTMLDivElement>()
  $effect(() => {
    if (open && menuEl && anchorEl) {
      const rect = anchorEl.getBoundingClientRect()
      menuEl.style.left = `${rect.left}px`
      menuEl.style.bottom = `${window.innerHeight - rect.top + 4}px`
    }
  })
</script>

{#if open}
  <div class="bp-scrim" role="presentation" onclick={onClose}></div>
  <div class="bp-menu gw-anim-slide-down" bind:this={menuEl} role="menu" aria-label="Branches">
    <button class="bp-item create" role="menuitem" onclick={createNew}>
      <Icon name="plus" size={13} /> Create new branch…
    </button>
    <div class="bp-sep"></div>
    {#if loading}
      <div class="bp-info">Loading…</div>
    {:else if branches.length === 0}
      <div class="bp-info">No branches</div>
    {:else}
      {#each branches as b (b)}
        <button
          class="bp-item"
          class:current={b === $git.branch}
          role="menuitem"
          onclick={() => switchTo(b)}
        >
          <span class="bp-check">{#if b === $git.branch}<Icon name="check" size={12} />{/if}</span>
          <span class="bp-name">{b}</span>
        </button>
      {/each}
    {/if}
    {#if busyMsg}
      <div class="bp-info busy">{busyMsg}</div>
    {/if}
  </div>
{/if}

<style>
  .bp-scrim {
    position: fixed;
    inset: 0;
    z-index: 1000;
  }
  .bp-menu {
    position: fixed;
    z-index: 1001;
    min-width: 220px;
    max-width: 320px;
    max-height: 60vh;
    overflow-y: auto;
    padding: 4px;
    background-color: var(--popover);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: var(--shadow-lg);
  }
  .bp-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 26px;
    padding: 0 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .bp-item:hover {
    background-color: var(--secondary);
  }
  .bp-item.create {
    color: var(--primary);
  }
  .bp-check {
    display: inline-flex;
    width: 14px;
    flex-shrink: 0;
    color: var(--primary);
  }
  .bp-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bp-item.current .bp-name {
    color: var(--primary);
  }
  .bp-sep {
    height: 1px;
    margin: 4px 4px;
    background-color: var(--border);
  }
  .bp-info {
    padding: 6px 8px;
    font-size: 12px;
    color: var(--muted-foreground);
  }
  .bp-info.busy {
    color: var(--primary);
  }
</style>
