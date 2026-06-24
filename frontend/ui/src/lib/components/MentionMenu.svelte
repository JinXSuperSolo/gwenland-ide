<script lang="ts">
  import type { MentionCandidate } from '../stores/mention-providers'
  import Icon from './Icon.svelte'

  /**
   * @-mention autocomplete (GWEN-332). A controlled, presentational dropdown
   * rendered ABOVE the composer — same anchor + visual language as
   * SlashCommandMenu. The parent owns the candidate list and the highlighted
   * index (so the textarea's Arrow/Enter keys drive it); we render rows + report
   * clicks/hover. Special providers (@git/@diagnostics/@terminal/@web) come first
   * in the list the parent passes; file/folder fuzzy results follow.
   */
  let {
    candidates,
    activeIndex,
    onSelect,
    onHover,
  }: {
    candidates: MentionCandidate[]
    activeIndex: number
    onSelect: (c: MentionCandidate) => void
    onHover: (index: number) => void
  } = $props()

  let listEl = $state<HTMLDivElement | null>(null)

  // Keep the highlighted row in view as the parent moves the selection.
  $effect(() => {
    const idx = activeIndex
    if (!listEl) return
    const row = listEl.children[idx] as HTMLElement | undefined
    row?.scrollIntoView({ block: 'nearest' })
  })
</script>

{#if candidates.length > 0}
  <div class="mention-menu" role="listbox" aria-label="Mentions" bind:this={listEl}>
    {#each candidates as c, i (c.type + ':' + c.insert)}
      <button
        type="button"
        role="option"
        aria-selected={i === activeIndex}
        class="mention-row"
        class:active={i === activeIndex}
        onmousemove={() => onHover(i)}
        onclick={() => onSelect(c)}
      >
        <span class="mention-icon"><Icon name={c.icon} size={14} /></span>
        <span class="mention-label">{c.label}</span>
        <span class="mention-detail">{c.detail}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .mention-menu {
    position: absolute;
    left: 0;
    right: 0;
    bottom: calc(100% + 6px);
    z-index: 40;
    max-height: 240px;
    overflow-y: auto;
    padding: 4px;
    background-color: var(--ai-bg-surface);
    border: 1px solid var(--ai-border-subtle, transparent);
    border-radius: 12px;
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.45));
    scrollbar-width: thin;
    scrollbar-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent)) transparent;
  }
  .mention-menu::-webkit-scrollbar {
    width: 4px;
  }
  .mention-menu::-webkit-scrollbar-thumb {
    background-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent));
    border-radius: 999px;
  }
  .mention-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: 7px;
    cursor: pointer;
    color: var(--ai-text-primary);
  }
  .mention-row.active {
    background-color: var(--ai-bg-hover);
  }
  .mention-icon {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--ai-primary-light);
  }
  .mention-label {
    flex-shrink: 0;
    font-size: 12px;
    font-weight: 600;
  }
  .mention-detail {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
    color: var(--ai-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: right;
  }
</style>
