<script lang="ts">
  import type { SlashCommand } from '../stores/slash-commands'

  /**
   * Compact slash-command autocomplete (GWEN-333). A controlled, presentational
   * dropdown rendered ABOVE the composer: the parent owns the filtered list and
   * the highlighted index (so the textarea's Arrow/Enter keys drive it), and we
   * just render rows + report clicks/hover. VSCode-feel — thin rows, no cards.
   */
  let {
    commands,
    activeIndex,
    onSelect,
    onHover,
  }: {
    commands: SlashCommand[]
    activeIndex: number
    onSelect: (cmd: SlashCommand) => void
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

{#if commands.length > 0}
  <div class="slash-menu" role="listbox" aria-label="Slash commands" bind:this={listEl}>
    {#each commands as cmd, i (cmd.id)}
      <button
        type="button"
        role="option"
        aria-selected={i === activeIndex}
        class="slash-row"
        class:active={i === activeIndex}
        onmousemove={() => onHover(i)}
        onclick={() => onSelect(cmd)}
      >
        <span class="slash-name">
          {cmd.name}{#if cmd.argHint}<span class="slash-arg"> {cmd.argHint}</span>{/if}
        </span>
        <span class="slash-desc">{cmd.description}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .slash-menu {
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
  .slash-menu::-webkit-scrollbar {
    width: 4px;
  }
  .slash-menu::-webkit-scrollbar-thumb {
    background-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent));
    border-radius: 999px;
  }
  .slash-row {
    display: flex;
    align-items: baseline;
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
  .slash-row.active {
    background-color: var(--ai-bg-hover);
  }
  .slash-name {
    flex-shrink: 0;
    font-family: var(--font-mono, monospace);
    font-size: 12px;
    font-weight: 600;
    color: var(--ai-primary-light);
  }
  .slash-arg {
    font-weight: 400;
    color: var(--ai-text-muted);
  }
  .slash-desc {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
    color: var(--ai-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
