<script lang="ts">
  import { paletteOpen, paletteInitialQuery, closePalette } from '../stores/ui'
  import {
    commandCategory,
    filterCommands,
    keybindingsFor,
    type Command,
  } from '../commands/registry'
  import { revealLine } from '../editor/active-editor'
  import Icon from './Icon.svelte'

  let query = $state('')
  let selected = $state(0)
  let inputEl = $state<HTMLInputElement>()

  // Recompute matches whenever the query or open-state changes.
  const matches = $derived.by(() => {
    if (!$paletteOpen) return []
    if (query.startsWith(':')) {
      const lineStr = query.slice(1)
      const lineNo = parseInt(lineStr, 10)
      const isNumber = !isNaN(lineNo)
      return [
        {
          id: 'goto-line',
          title: isNumber ? `Go to Line ${lineNo}` : 'Type a line number to navigate to...',
          category: 'Navigation',
          handler: () => {
            if (isNumber) {
              revealLine(lineNo)
            }
          },
        },
      ]
    }
    return filterCommands(query)
  })

  // Reset + focus when the palette opens.
  $effect(() => {
    if ($paletteOpen) {
      query = $paletteInitialQuery
      selected = 0
      // Focus after the input is in the DOM.
      queueMicrotask(() => inputEl?.focus())
    }
  })

  // Keep the selection in range as matches shrink.
  $effect(() => {
    if (selected >= matches.length) selected = Math.max(0, matches.length - 1)
  })

  function run(cmd: Command | undefined) {
    if (!cmd) return
    closePalette()
    void cmd.handler()
  }

  function onKeydown(e: KeyboardEvent) {
    // Escape is owned by the centralized overlay stack (App.svelte → closeTopmost);
    // letting it bubble keeps "one press = one layer" consistent.
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      selected = Math.min(selected + 1, matches.length - 1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      selected = Math.max(selected - 1, 0)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      run(matches[selected])
    }
  }

  function keyChips(combo: string): string[] {
    return combo ? combo.split('+') : []
  }
</script>

{#if $paletteOpen}
  <div
    class="palette-overlay gw-anim-overlay"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) closePalette()
    }}
  >
    <div class="palette gw-anim-pop" role="dialog" aria-modal="true" aria-label="Command Palette">
      <div class="palette-search">
        <Icon name="search" size={16} class="palette-search-icon" />
        <input
          class="palette-input"
          bind:this={inputEl}
          bind:value={query}
          placeholder={query.startsWith(':') ? 'Type line number...' : 'Type a command…'}
          aria-label="Command search"
          onkeydown={onKeydown}
        />
      </div>
      <div class="palette-results" role="listbox">
        {#each matches as cmd, i (cmd.id)}
          <button
            type="button"
            class="palette-item"
            class:selected={i === selected}
            role="option"
            aria-selected={i === selected}
            onmousemove={() => (selected = i)}
            onclick={() => run(cmd)}
          >
            <span class="palette-cat">{cmd.id === 'goto-line' ? 'Navigation' : commandCategory(cmd.id)}</span>
            <span class="palette-label">{cmd.title}</span>
            <span class="palette-keys">
              {#each keyChips(keybindingsFor(cmd)[0] ?? '') as k}
                <kbd class="palette-kbd">{k}</kbd>
              {/each}
            </span>
          </button>
        {/each}
        {#if matches.length === 0}
          <div class="palette-empty">No matching commands</div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .palette-overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    justify-content: center;
    align-items: center;
    background-color: rgba(0, 0, 0, 0.4);
  }
  .palette {
    width: 560px;
    max-width: 90vw;
    background-color: var(--popover);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-xl);
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .palette-search {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 14px;
    border-bottom: 1px solid var(--border);
    background-color: var(--input);
  }
  .palette-search :global(.palette-search-icon) {
    color: var(--muted-foreground);
    flex-shrink: 0;
  }
  .palette-input {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 14px;
    padding: 12px 0;
    outline: none;
  }
  .palette-results {
    max-height: 360px;
    overflow-y: auto;
    padding: 4px;
  }
  .palette-item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    transition: background-color 0.12s ease, color 0.12s ease;
  }
  .palette-item.selected {
    background-color: var(--primary);
    color: var(--primary-foreground);
  }
  .palette-cat {
    font-size: 11px;
    color: var(--muted-foreground);
    min-width: 80px;
    flex-shrink: 0;
  }
  .palette-item.selected .palette-cat {
    color: var(--primary-foreground);
    opacity: 0.8;
  }
  .palette-label {
    flex: 1;
  }
  .palette-keys {
    display: flex;
    gap: 3px;
    flex-shrink: 0;
  }
  .palette-kbd {
    background-color: var(--secondary);
    color: var(--secondary-foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    font-size: 10px;
    font-family: var(--font-mono);
  }
  .palette-item.selected .palette-kbd {
    background-color: color-mix(in srgb, var(--primary-foreground) 20%, transparent);
    border-color: transparent;
    color: var(--primary-foreground);
  }
  .palette-empty {
    padding: 16px;
    text-align: center;
    color: var(--muted-foreground);
    font-size: 13px;
  }
</style>
