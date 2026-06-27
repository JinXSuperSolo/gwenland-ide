<script lang="ts">
  import { sidebarView, type SidebarView } from '../stores/sidebar'
  import { git } from '../stores/git'
  import { togglePanel } from '../stores/panels'
  import Icon from './Icon.svelte'
  import type { IconName } from './Icon.svelte'

  // Thin vertical rail switching the left sidebar between Explorer and Source
  // Control (GWEN-328). Source Control hides when the folder isn't a git repo.
  function select(view: SidebarView) {
    // Clicking the active view toggles the panel collapsed/expanded.
    if ($sidebarView === view) {
      togglePanel('fileTree')
    } else {
      sidebarView.set(view)
    }
  }

  const items = $derived(
    [
      { view: 'explorer' as const, icon: 'page' as IconName, label: 'Explorer' },
      { view: 'search' as const, icon: 'search' as IconName, label: 'Search' },
      ...($git.isRepo
        ? [{ view: 'git' as const, icon: 'git-branch' as IconName, label: 'Source Control' }]
        : []),
    ]
  )
</script>

<nav class="activity-bar" aria-label="Activity Bar">
  {#each items as item (item.view)}
    <button
      class="act"
      class:active={$sidebarView === item.view}
      title={item.label}
      aria-label={item.label}
      aria-pressed={$sidebarView === item.view}
      onclick={() => select(item.view)}
    >
      <Icon name={item.icon} size={20} />
      {#if item.view === 'git' && $git.dirtyCount > 0}
        <span class="act-badge">{$git.dirtyCount}</span>
      {/if}
    </button>
  {/each}
</nav>

<style>
  .activity-bar {
    width: 44px;
    flex-shrink: 0;
    height: 100%;
    background-color: var(--background);
    border-right: 1px solid var(--sidebar-border);
    display: flex;
    flex-direction: column;
    align-items: center;
    padding-top: 6px;
    gap: 2px;
  }
  .act {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
    transition: color 0.12s ease, background-color 0.12s ease;
  }
  .act:hover {
    color: var(--foreground);
    background-color: var(--sidebar-accent);
  }
  .act.active {
    color: var(--primary);
  }
  .act.active::before {
    content: '';
    position: absolute;
    left: -6px;
    top: 8px;
    bottom: 8px;
    width: 2px;
    border-radius: 2px;
    background-color: var(--primary);
  }
  .act-badge {
    position: absolute;
    bottom: 4px;
    right: 4px;
    min-width: 14px;
    height: 14px;
    padding: 0 3px;
    font-size: 9px;
    line-height: 14px;
    text-align: center;
    color: var(--primary-foreground);
    background-color: var(--primary);
    border-radius: 999px;
  }
</style>
