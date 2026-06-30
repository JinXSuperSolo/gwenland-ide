<script lang="ts">
  import { aiChat } from '../stores/ai-chat'
  import { panels } from '../stores/panels'
  import { sidebarTab, sidebarView, type SidebarTab } from '../stores/sidebar'
  import AiPanel from './AiPanel.svelte'
  import FileTree from './FileTree.svelte'
  import Icon from './Icon.svelte'
  import type { IconName } from './Icon.svelte'

  const tabs: { id: SidebarTab; label: string; icon: IconName }[] = [
    { id: 'files', label: 'Files', icon: 'page' },
    { id: 'agent', label: 'Agent', icon: 'sparks' },
  ]

  $effect(() => {
    const isAgentVisible =
      $sidebarView === 'explorer' && $sidebarTab === 'agent' && !$panels.fileTree.collapsed
    aiChat.update((state) => (state.isOpen === isAgentVisible ? state : { ...state, isOpen: isAgentVisible }))
  })

  function selectTab(tab: SidebarTab): void {
    sidebarTab.set(tab)
  }
</script>

<aside class="sidebar-tabs" aria-label="Sidebar">
  <div class="tab-body">
    {#if $sidebarTab === 'agent'}
      <AiPanel />
    {:else}
      <FileTree />
    {/if}
  </div>

  <nav class="tab-bar" aria-label="Sidebar Tabs">
    {#each tabs as tab (tab.id)}
      <button
        type="button"
        class="tab-button btn-compact gw-transition"
        class:active={$sidebarTab === tab.id}
        aria-label={tab.label}
        aria-pressed={$sidebarTab === tab.id}
        onclick={() => selectTab(tab.id)}
      >
        <Icon name={tab.icon} size={15} />
        <span>{tab.label}</span>
      </button>
    {/each}

    <div class="tab-wrap disabled">
      <button type="button" class="tab-button btn-compact gw-transition disabled-button" disabled aria-label="Agent 2.0">
        <Icon name="brain" size={15} />
        <span>Agent 2.0</span>
      </button>
      <span class="coming-tooltip" role="tooltip">Coming Soon</span>
    </div>
  </nav>
</aside>

<style>
  .sidebar-tabs {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
    background: var(--background);
    color: var(--foreground);
  }

  .tab-body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .tab-bar {
    flex-shrink: 0;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 3px;
    padding: 4px;
    border-top: 1px solid var(--sidebar-border);
    background: var(--background);
  }

  .tab-wrap {
    position: relative;
    min-width: 0;
  }

  .tab-button {
    width: 100%;
    min-width: 0;
    background: transparent;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-weight: 700;
  }

  .tab-button span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tab-button:not(.active):hover:not(:disabled) {
    color: var(--foreground);
    background: var(--hover-bg);
  }

  .tab-button.active {
    background: var(--primary);
    color: var(--primary-foreground);
    border: none;
    outline: none;
  }

  .disabled-button {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .coming-tooltip {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 8px);
    z-index: 20;
    transform: translateX(-50%);
    pointer-events: none;
    opacity: 0;
    white-space: nowrap;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--popover);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 11px;
    box-shadow: var(--shadow-md);
    transition: opacity 0.12s ease;
  }

  .coming-tooltip::after {
    content: '';
    position: absolute;
    left: 50%;
    top: 100%;
    transform: translateX(-50%);
    border: 5px solid transparent;
    border-top-color: var(--popover);
  }

  .tab-wrap.disabled:hover .coming-tooltip {
    opacity: 1;
  }
</style>
