<script lang="ts">
  import { activateTab, setActiveGroup, type Tab } from '../stores/tabs'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import Icon from './Icon.svelte'
  import FileIcon from './FileIcon.svelte'

  // Close is handled by the parent (Workspace) so it can gate dirty tabs with a
  // confirm before discarding. We just emit the intent.
  let {
    groupId,
    tabs: groupTabs,
    activeId,
    onClose,
  }: {
    groupId: string
    tabs: Tab[]
    activeId: string | null
    onClose: (id: string) => void
  } = $props()

  // Hover tooltip: an editor tab's on-disk path, a diff tab's path, or a
  // preview's source target.
  function tabTitle(tab: Tab): string {
    if (tab.kind === 'editor') return tab.path
    if (tab.kind === 'diff') return `${tab.path} (diff)`
    return tab.source.kind === 'static-file' ? tab.source.path : tab.source.url
  }

  // Right-click a tab opens the editor-tab context menu (M9). Only editor tabs
  // carry a file path; preview tabs pass none, so path-gated actions disable.
  function onTabContextMenu(e: MouseEvent, tab: Tab) {
    openContextMenu(e, {
        scope: 'editor_tab',
        tabId: tab.id,
        groupId,
        path: tab.kind === 'editor' ? tab.path || undefined : undefined,
      })
  }
</script>

{#if groupTabs.length > 0}
  <div class="tabs-bar" role="tablist">
    {#each groupTabs as tab (tab.id)}
      <div
        class="tab-item gw-anim-rise"
        class:active={tab.id === activeId}
        class:dirty={tab.kind === 'editor' && tab.dirty}
        class:preview={tab.preview}
        role="tab"
        aria-selected={tab.id === activeId}
        tabindex="0"
        title={tabTitle(tab)}
        onmousedown={() => activateTab(tab.id, groupId)}
        oncontextmenu={(e) => onTabContextMenu(e, tab)}
        onkeydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            setActiveGroup(groupId)
            activateTab(tab.id, groupId)
          }
        }}
      >
        <FileIcon name={tab.name} size={15} />
        <span class="tab-title">{tab.name}</span>
        <span class="tab-dirty-dot"></span>
        <button
          type="button"
          class="tab-close"
          title="Close"
          aria-label={`Close ${tab.name}`}
          onmousedown={(e) => e.stopPropagation()}
          onclick={(e) => {
            e.stopPropagation()
            onClose(tab.id)
          }}
        ><Icon name="xmark" size={13} /></button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .tabs-bar {
    height: 36px;
    flex-shrink: 0;
    background-color: var(--background);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: stretch;
    overflow-x: auto;
    overflow-y: hidden;
  }
  .tabs-bar::-webkit-scrollbar {
    height: 0;
  }
  .tab-item {
    display: flex;
    align-items: center;
    padding: 0 12px;
    background-color: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 13px;
    color: var(--muted-foreground);
    cursor: pointer;
    gap: 7px;
    transition: color 0.15s ease, background-color 0.15s ease;
    max-width: 180px;
    position: relative;
    white-space: nowrap;
  }
  .tab-item:hover {
    color: var(--foreground);
    background-color: var(--secondary);
  }
  .tab-item.active {
    color: var(--foreground);
    background-color: var(--card);
    border-bottom: 2px solid var(--primary);
    font-weight: 500;
  }
  .tab-item.preview .tab-title {
    font-style: italic;
  }
  .tab-title {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tab-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    padding: 0;
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
    border-radius: var(--radius-sm);
    opacity: 0;
    font-size: 14px;
    line-height: 1;
    transition: opacity 0.15s ease, background-color 0.15s ease;
  }
  .tab-item:hover .tab-close {
    opacity: 1;
  }
  .tab-close:hover {
    background-color: var(--border);
  }
  /* Dirty indicator: a filled dot occupies the close slot until hovered. */
  .tab-dirty-dot {
    display: none;
    width: 16px;
    height: 16px;
    align-items: center;
    justify-content: center;
  }
  .tab-dirty-dot::before {
    content: '';
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background-color: var(--primary);
  }
  .tab-item.dirty .tab-dirty-dot {
    display: inline-flex;
  }
  .tab-item.dirty .tab-close {
    display: none;
  }
  .tab-item.dirty:hover .tab-dirty-dot {
    display: none;
  }
  .tab-item.dirty:hover .tab-close {
    display: inline-flex;
    opacity: 1;
  }
</style>
