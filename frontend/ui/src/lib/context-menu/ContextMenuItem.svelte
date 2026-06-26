<script lang="ts">
  import { isActionEnabled } from './actionRegistry'
  import { closeContextMenu } from './contextMenuStore'
  import { shortcutFor } from '../commands/registry'
  import type { ContextAction, ContextMenuContext } from './contextTypes'

  let { action, ctx }: { action: ContextAction; ctx: ContextMenuContext } = $props()

  const enabled = $derived(isActionEnabled(action, ctx))
  // Prefer the live command-registry shortcut (Task 5.2), fall back to the
  // action's static hint.
  const shortcut = $derived(
    (action.commandId ? shortcutFor(action.commandId) : undefined) ?? action.shortcut,
  )

  async function activate() {
    if (!enabled) return
    // Close first so a slow/awaited action doesn't leave the menu hanging open.
    closeContextMenu()
    try {
      await action.run(ctx)
    } catch (e) {
      console.error(`context action "${action.id}" failed:`, e)
    }
  }
</script>

<button
  type="button"
  class="cm-item"
  class:cm-danger={action.danger}
  role="menuitem"
  tabindex="-1"
  disabled={!enabled}
  aria-disabled={!enabled}
  data-cm-item
  onclick={activate}
>
  <span class="cm-item-label">{action.label}</span>
  {#if shortcut}
    <span class="cm-item-shortcut">{shortcut}</span>
  {/if}
</button>

<style>
  /* GWEN-322: VS Code-compact rows. Label left, muted shortcut right. 22-24px
     tall, no bold, square corners (the container owns the rounding). Hover is a
     subtle dark surface lift, NOT a full accent block. */
  .cm-item {
    display: flex;
    align-items: center;
    gap: 32px;
    width: 100%;
    height: 26px;
    padding: 0 8px;
    background: transparent;
    border: none;
    border-radius: calc(var(--radius) - 6px);
    color: var(--cm-text);
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 400;
    text-align: left;
    cursor: pointer;
    white-space: nowrap;
  }
  .cm-item:hover:not(:disabled),
  .cm-item:focus-visible:not(:disabled) {
    background-color: var(--cm-item-hover);
    color: var(--cm-item-hover-text);
    outline: none;
  }
  .cm-item:active:not(:disabled) {
    background-color: var(--cm-item-active);
  }
  .cm-item:disabled {
    color: var(--cm-text-muted);
    cursor: default;
    pointer-events: none;
  }
  /* Danger / destructive actions — red label, red-tinted hover. */
  .cm-item.cm-danger {
    color: #e06c75;
  }
  .cm-item.cm-danger:hover:not(:disabled),
  .cm-item.cm-danger:focus-visible:not(:disabled) {
    background-color: rgba(224, 108, 117, 0.15);
    color: #e06c75;
  }
  .cm-item.cm-danger:active:not(:disabled) {
    background-color: rgba(224, 108, 117, 0.25);
  }
  .cm-item-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cm-item-shortcut {
    flex-shrink: 0;
    font-size: 11px;
    color: var(--cm-shortcut-text);
  }
</style>
