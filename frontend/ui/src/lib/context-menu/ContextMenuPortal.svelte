<script lang="ts">
  import { registry } from './actionRegistry'
  import { contextMenuStore, closeContextMenu } from './contextMenuStore'
  import { contextMenuKeyNav } from './keybindings'
  import ContextMenuItem from './ContextMenuItem.svelte'
  import ContextMenuSeparator from './ContextMenuSeparator.svelte'

  let menuEl = $state<HTMLDivElement>()

  // Grouped actions for the active context; empty when closed.
  const groups = $derived(
    $contextMenuStore.open && $contextMenuStore.context
      ? registry.getGrouped($contextMenuStore.context)
      : [],
  )

  /** Move the menu out to <body> so no panel's overflow/stacking can clip it. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node)
    return {
      destroy() {
        if (node.parentNode) node.parentNode.removeChild(node)
      },
    }
  }

  // Position at the pointer, then clamp into the viewport once measured
  // (Requirement 2.2 / 2.3). Runs again whenever x/y change.
  $effect(() => {
    const { open, x, y } = $contextMenuStore
    if (!open || !menuEl) return
    const margin = 8
    const rect = menuEl.getBoundingClientRect()
    let left = x
    let top = y
    if (left + rect.width > window.innerWidth - margin) {
      left = window.innerWidth - rect.width - margin
    }
    if (top + rect.height > window.innerHeight - margin) {
      top = window.innerHeight - rect.height - margin
    }
    menuEl.style.left = `${Math.max(margin, left)}px`
    menuEl.style.top = `${Math.max(margin, top)}px`
  })

  // Focus the menu container on open so arrow-key navigation works immediately
  // without pre-highlighting the first item (Requirement 3.1). preventScroll so
  // focusing can never nudge a scroll container (which would self-dismiss).
  $effect(() => {
    if ($contextMenuStore.open && menuEl) {
      const el = menuEl
      queueMicrotask(() => el.focus({ preventScroll: true }))
    }
  })

  // Dismissal: outside pointer-down, window resize, and any outside scroll
  // (Requirement 2.4). Capture phase so it sees events on any element. The
  // listeners are attached on the NEXT tick so the very right-click / pointer
  // gesture that opened the menu can never immediately close it again.
  $effect(() => {
    if (!$contextMenuStore.open) return
    function onPointerDown(e: PointerEvent) {
      if (menuEl && !menuEl.contains(e.target as Node)) closeContextMenu()
    }
    function onResize() {
      closeContextMenu()
    }
    function onScroll(e: Event) {
      // Ignore scrolling that happens inside the menu itself.
      if (menuEl && menuEl.contains(e.target as Node)) return
      closeContextMenu()
    }
    const armId = window.setTimeout(() => {
      window.addEventListener('pointerdown', onPointerDown, true)
      window.addEventListener('resize', onResize)
      window.addEventListener('scroll', onScroll, true)
    }, 0)
    return () => {
      window.clearTimeout(armId)
      window.removeEventListener('pointerdown', onPointerDown, true)
      window.removeEventListener('resize', onResize)
      window.removeEventListener('scroll', onScroll, true)
    }
  })
</script>

{#if $contextMenuStore.open && $contextMenuStore.context}
  <div
    use:portal
    bind:this={menuEl}
    class="context-menu cm-menu cm-anim"
    role="menu"
    aria-orientation="vertical"
    tabindex="-1"
    style:left={`${$contextMenuStore.x}px`}
    style:top={`${$contextMenuStore.y}px`}
    use:contextMenuKeyNav
    oncontextmenu={(e) => {
      e.preventDefault()
      e.stopPropagation()
    }}
  >
    {#each groups as group, i (group.group)}
      {#if i > 0}
        <ContextMenuSeparator />
      {/if}
      {#each group.actions as action (action.id)}
        <ContextMenuItem {action} ctx={$contextMenuStore.context} />
      {/each}
    {/each}
    {#if groups.length === 0}
      <div class="cm-empty">No actions</div>
    {/if}
  </div>
{/if}

<style>
  .cm-menu {
    /* Visual tokens, scoped to the menu subtree (design §Visual Tokens).
       Hover/focus uses the brand accent as a solid highlight (VS Code-style). */
    --cm-bg: #1f1d1c;
    --cm-item-hover: var(--primary);
    --cm-item-hover-text: var(--primary-foreground);
    --cm-item-active: color-mix(in srgb, var(--primary) 85%, #000);
    --cm-text: #e8e0d8;
    --cm-text-muted: rgba(232, 224, 216, 0.45);
    --cm-separator: rgba(255, 255, 255, 0.08);
    --cm-radius: 8px;
    --cm-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
    --cm-shortcut-text: rgba(232, 224, 216, 0.42);

    position: fixed;
    z-index: 1000;
    min-width: 220px;
    max-width: 320px;
    padding: 6px;
    border: 1px solid rgba(255, 255, 255, 0.06);
    background-color: var(--cm-bg);
    border-radius: var(--cm-radius);
    box-shadow: var(--cm-shadow);
    outline: none;
    display: flex;
    flex-direction: column;
    font-family: var(--font-sans);
    user-select: none;
  }
  .cm-anim {
    transform-origin: top left;
    animation: cm-pop 0.1s ease-out;
  }
  @keyframes cm-pop {
    from {
      opacity: 0;
      transform: scale(0.96);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }
  .cm-empty {
    padding: 8px 10px;
    font-size: 13px;
    color: var(--cm-text-muted);
  }
  @media (prefers-reduced-motion: reduce) {
    .cm-anim {
      animation: none;
    }
  }
</style>
