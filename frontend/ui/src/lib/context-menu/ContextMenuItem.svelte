<script lang="ts">
  import { isActionEnabled } from './actionRegistry'
  import { closeContextMenu } from './contextMenuStore'
  import { shortcutFor } from '../stores/commands'
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
  /* Icon-free rows: label left, muted shortcut right (VS Code-style). */
  .cm-item {
    display: flex;
    align-items: center;
    gap: 32px;
    width: 100%;
    min-height: 28px;
    padding: 0 14px;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--cm-text);
    font-family: var(--font-sans);
    font-size: 13px;
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
    color: var(--cm-item-hover-text);
  }
  .cm-item:disabled {
    color: var(--cm-text-muted);
    cursor: default;
    pointer-events: none;
  }
  .cm-item-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cm-item-shortcut {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--cm-shortcut-text);
  }
  /* On the accent highlight the shortcut stays readable. */
  .cm-item:hover:not(:disabled) .cm-item-shortcut,
  .cm-item:focus-visible:not(:disabled) .cm-item-shortcut {
    color: var(--cm-item-hover-text);
  }
</style>
