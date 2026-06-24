<script lang="ts">
  import { assistantMode, MODE_META, ASSISTANT_MODES, type AssistantMode } from '../stores/assistant-mode'
  import Icon, { type IconName } from './Icon.svelte'

  // Cursor-style mode picker for the unified assistant panel (M10 Wave 9). Lives
  // in the composer toolbar; `placement="up"` opens the menu above the trigger.
  let { placement = 'down' }: { placement?: 'up' | 'down' } = $props()

  let open = $state(false)
  let wrapEl = $state<HTMLDivElement>()

  function pick(mode: AssistantMode) {
    assistantMode.set(mode)
    open = false
  }

  // Close on outside pointer-down while open.
  $effect(() => {
    if (!open) return
    function onPointerDown(e: PointerEvent) {
      if (wrapEl && !wrapEl.contains(e.target as Node)) open = false
    }
    window.addEventListener('pointerdown', onPointerDown, true)
    return () => window.removeEventListener('pointerdown', onPointerDown, true)
  })
</script>

<div class="mode-wrap" bind:this={wrapEl}>
  <button
    type="button"
    class="mode-trigger"
    aria-haspopup="menu"
    aria-expanded={open}
    title="Assistant mode"
    onclick={() => (open = !open)}
  >
    <Icon name={MODE_META[$assistantMode].icon as IconName} size={13} />
    <span class="mode-trigger-label">{MODE_META[$assistantMode].label}</span>
    <Icon name="nav-arrow-down" size={12} />
  </button>

  {#if open}
    <div class="mode-menu" class:up={placement === 'up'} role="menu" tabindex="-1" onkeydown={(e) => { if (e.key === 'Escape') open = false }}>
      {#each ASSISTANT_MODES as mode (mode)}
        <button
          type="button"
          class="mode-item"
          class:active={mode === $assistantMode}
          role="menuitemradio"
          aria-checked={mode === $assistantMode}
          onclick={() => pick(mode)}
        >
          <span class="mode-item-icon"><Icon name={MODE_META[mode].icon as IconName} size={15} /></span>
          <span class="mode-item-text">
            <span class="mode-item-label">{MODE_META[mode].label}</span>
            <span class="mode-item-hint">{MODE_META[mode].hint}</span>
          </span>
          {#if mode === $assistantMode}
            <Icon name="check" size={14} />
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .mode-wrap {
    position: relative;
    display: inline-flex;
  }
  .mode-trigger {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--ai-border-subtle);
    border-radius: 999px;
    background: color-mix(in srgb, var(--ai-bg-surface) 82%, transparent);
    color: var(--ai-text-primary);
    font: 600 11px var(--font-sans);
    cursor: pointer;
    transition: background-color 0.12s ease, border-color 0.12s ease;
  }
  .mode-trigger:hover {
    border-color: color-mix(in srgb, var(--ai-primary) 35%, transparent);
  }
  .mode-trigger-label {
    line-height: 1;
  }
  .mode-menu {
    position: absolute;
    top: 30px;
    left: 0;
    z-index: 50;
    min-width: 230px;
    padding: 4px;
    background-color: var(--ai-bg-surface);
    border: 1px solid var(--ai-border-subtle);
    border-radius: 12px;
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.45));
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .mode-menu.up {
    top: auto;
    bottom: 30px;
  }
  .mode-item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 7px 9px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--ai-text-primary);
    text-align: left;
    cursor: pointer;
  }
  .mode-item:hover {
    background-color: var(--ai-bg-hover);
  }
  .mode-item.active {
    background-color: color-mix(in srgb, var(--ai-primary) 14%, transparent);
  }
  .mode-item-icon {
    display: inline-flex;
    color: var(--ai-primary);
    flex-shrink: 0;
  }
  .mode-item-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }
  .mode-item-label {
    font: 600 12px var(--font-sans);
  }
  .mode-item-hint {
    font-size: 11px;
    color: var(--ai-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
