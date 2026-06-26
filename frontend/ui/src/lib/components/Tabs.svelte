<script lang="ts">
  import { activateTab, moveTabToGroup, setActiveGroup, type Tab } from '../stores/tabs'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import Icon from './Icon.svelte'
  import FileIcon from './FileIcon.svelte'

  const TAB_DRAG_MIME = 'application/x-gwenland-tab'

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

  let tabsBar: HTMLDivElement | null = null
  let dragOver = $state(false)
  let draggingId = $state<string | null>(null)

  $effect(() => {
    const id = activeId
    groupTabs.length
    if (!id || !tabsBar) return
    requestAnimationFrame(() => {
      tabsBar?.querySelector<HTMLElement>('.tab-item.active')?.scrollIntoView({
        block: 'nearest',
        inline: 'nearest',
      })
    })
  })

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

  function dragPayload(e: DragEvent): { tabId: string } | null {
    const raw = e.dataTransfer?.getData(TAB_DRAG_MIME) || e.dataTransfer?.getData('text/plain')
    if (!raw) return null
    try {
      const parsed = JSON.parse(raw)
      return typeof parsed.tabId === 'string' ? { tabId: parsed.tabId } : null
    } catch {
      return null
    }
  }

  function hasTabDrag(e: DragEvent): boolean {
    return Array.from(e.dataTransfer?.types ?? []).includes(TAB_DRAG_MIME)
  }

  function dropIndex(e: DragEvent): number {
    const target = e.target instanceof HTMLElement ? e.target.closest<HTMLElement>('.tab-item') : null
    if (!target || !tabsBar?.contains(target)) return groupTabs.length
    const id = target.dataset.tabId
    const index = groupTabs.findIndex((tab) => tab.id === id)
    if (index < 0) return groupTabs.length
    const rect = target.getBoundingClientRect()
    return e.clientX < rect.left + rect.width / 2 ? index : index + 1
  }

  function onDragStart(e: DragEvent, tab: Tab) {
    draggingId = tab.id
    const payload = JSON.stringify({ tabId: tab.id })
    e.dataTransfer?.setData(TAB_DRAG_MIME, payload)
    e.dataTransfer?.setData('text/plain', payload)
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
  }

  function onDragOver(e: DragEvent) {
    if (!hasTabDrag(e)) return
    e.preventDefault()
    dragOver = true
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
  }

  function onDrop(e: DragEvent) {
    const payload = dragPayload(e)
    if (!payload) return
    e.preventDefault()
    dragOver = false
    moveTabToGroup(payload.tabId, groupId, dropIndex(e))
  }
</script>

<div
  class="tabs-bar"
  class:empty={groupTabs.length === 0}
  class:drag-over={dragOver}
  role="tablist"
  tabindex="-1"
  bind:this={tabsBar}
  ondragover={onDragOver}
  ondragleave={(e) => {
    if (e.currentTarget === e.target) dragOver = false
  }}
  ondrop={onDrop}
>
  {#each groupTabs as tab (tab.id)}
    <div
      class="tab-item gw-anim-rise"
      class:active={tab.id === activeId}
      class:dirty={tab.kind === 'editor' && tab.dirty}
      class:preview={tab.preview}
      class:dragging={draggingId === tab.id}
      data-tab-id={tab.id}
      role="tab"
      aria-selected={tab.id === activeId}
      tabindex="0"
      title={tabTitle(tab)}
      draggable="true"
      ondragstart={(e) => onDragStart(e, tab)}
      ondragend={() => {
        draggingId = null
        dragOver = false
      }}
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
        draggable="false"
        onmousedown={(e) => e.stopPropagation()}
        onclick={(e) => {
          e.stopPropagation()
          onClose(tab.id)
        }}
      ><Icon name="xmark" size={13} /></button>
    </div>
  {/each}
</div>

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
    min-width: 0;
    scrollbar-width: none;
  }
  .tabs-bar.drag-over {
    background-color: color-mix(in srgb, var(--primary) 8%, var(--background));
  }
  .tabs-bar::-webkit-scrollbar {
    height: 0;
  }
  .tab-item {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    min-width: 96px;
    max-width: 200px;
    padding: 0 12px;
    background-color: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 13px;
    color: var(--muted-foreground);
    cursor: pointer;
    gap: 7px;
    transition: color 0.15s ease, background-color 0.15s ease;
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
  .tab-item.dragging {
    opacity: 0.45;
  }
  .tab-title {
    min-width: 0;
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
