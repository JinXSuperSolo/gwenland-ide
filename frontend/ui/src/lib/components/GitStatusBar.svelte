<script lang="ts">
  import { git } from '../stores/git'
  import { showSidebarView } from '../stores/sidebar'
  import Icon from './Icon.svelte'
</script>

{#if $git.isRepo}
  <div class="git-status">
    <button
      class="git-branch"
      title="Open Source Control"
      aria-label={`Branch: ${$git.branch}`}
      onclick={() => showSidebarView('git')}
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
