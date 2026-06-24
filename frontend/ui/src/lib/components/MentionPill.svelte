<script lang="ts">
  import { iconForMention, type MentionItem } from '../stores/mention-providers'
  import { openFile } from '../stores/tabs'
  import { selectRange } from '../editor/active-editor'
  import { tick } from 'svelte'
  import Icon from './Icon.svelte'

  /**
   * An interactive @-mention chip in the composer (GWEN-332). Shows a file-type
   * icon + the short label and an × to remove it. Clicking a file/folder pill
   * opens it in an editor tab; for a `@file:start-end` pill the range is then
   * selected in CodeMirror. Non-file pills (@git/@diagnostics/@terminal/@web)
   * are display-only — clicking does nothing, but they're still removable.
   */
  let { mention, onRemove }: { mention: MentionItem; onRemove: () => void } = $props()

  const clickable = $derived(mention.type === 'file' || mention.type === 'folder')
  const icon = $derived(iconForMention(mention))

  async function onClick() {
    if (!mention.path) return
    if (mention.type === 'folder') {
      // No editor view for a folder; opening is a no-op (still removable).
      return
    }
    const res = await openFile(mention.path)
    if (!res.ok) return
    // Let the editor mount/activate before selecting the range.
    if (mention.lStart !== undefined && mention.lEnd !== undefined) {
      await tick()
      requestAnimationFrame(() => selectRange(mention.lStart!, mention.lEnd!))
    }
  }
</script>

<span class="pill" class:clickable>
  {#if clickable && mention.type === 'file'}
    <button type="button" class="pill-body" title={mention.path} onclick={onClick}>
      <Icon name={icon} size={11} />
      <span class="pill-label">{mention.label}</span>
    </button>
  {:else}
    <span class="pill-body static">
      <Icon name={icon} size={11} />
      <span class="pill-label">{mention.label}</span>
    </span>
  {/if}
  <button type="button" class="pill-x" aria-label={`Remove ${mention.label}`} onclick={onRemove}>
    <Icon name="xmark" size={11} />
  </button>
</span>

<style>
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    max-width: 100%;
    padding: 1px 3px 1px 2px;
    font-size: 11px;
    background-color: var(--ai-bg-hover);
    border: 1px solid var(--ai-border-subtle);
    border-radius: 999px;
    color: var(--ai-text-primary);
  }
  .pill-body {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    padding: 1px 5px;
    background: transparent;
    border: none;
    border-radius: 999px;
    color: inherit;
    font-size: inherit;
    font-family: var(--font-sans);
  }
  .pill.clickable .pill-body {
    cursor: pointer;
  }
  .pill.clickable .pill-body:hover {
    color: var(--ai-primary-light);
  }
  .pill-body.static {
    cursor: default;
  }
  .pill-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 180px;
  }
  .pill-x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: var(--ai-text-muted);
    cursor: pointer;
    padding: 0;
  }
  .pill-x:hover {
    color: var(--ai-text-primary);
    background-color: rgba(255, 255, 255, 0.08);
  }
</style>
