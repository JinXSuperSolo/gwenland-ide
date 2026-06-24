<script lang="ts">
  import { git } from '../stores/git'
  import { showSidebarView } from '../stores/sidebar'
  import BranchSwitcher from './BranchSwitcher.svelte'
  import Icon from './Icon.svelte'

  // GWEN-327: branch + dirty-count indicator, hidden when not a git repo.
  // Clicking the branch opens the branch picker (GWEN-331); clicking the dirty
  // count opens the Source Control panel (GWEN-328).
  let branchPickerOpen = $state(false)
  let anchorEl = $state<HTMLButtonElement>()
</script>

{#if $git.isRepo}
  <div class="git-status">
    <button
      class="git-branch"
      bind:this={anchorEl}
      title="Switch branch"
      aria-label={`Branch: ${$git.branch}`}
      onclick={() => (branchPickerOpen = !branchPickerOpen)}
    >
      <Icon name="git-branch" size={12} />
      <span class="git-branch-name">{$git.branch}</span>
    </button>
    {#if $git.dirtyCount > 0}
      <button
        class="git-dirty"
        title={`${$git.dirtyCount} change${$git.dirtyCount === 1 ? '' : 's'} — open Source Control`}
        aria-label={`${$git.dirtyCount} changes`}
        onclick={() => showSidebarView('git')}
      >
        <span class="dot">●</span>{$git.dirtyCount}
      </button>
    {/if}
  </div>

  <BranchSwitcher
    open={branchPickerOpen}
    {anchorEl}
    onClose={() => (branchPickerOpen = false)}
  />
{/if}

<style>
  .git-status {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .git-branch,
  .git-dirty {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 18px;
    padding: 0 6px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 11px;
    cursor: pointer;
    transition: color 0.12s ease, background-color 0.12s ease;
  }
  .git-branch:hover,
  .git-dirty:hover {
    color: var(--foreground);
    background-color: var(--secondary);
  }
  .git-branch-name {
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .git-dirty .dot {
    color: var(--primary);
    font-size: 8px;
    line-height: 1;
  }
</style>
