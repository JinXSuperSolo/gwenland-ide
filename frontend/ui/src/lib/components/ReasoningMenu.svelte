<script lang="ts">
  import { aiChat, setReasoningLevel } from '../stores/ai-chat'
  import { aiModelCatalog, type ModelEntry } from '../tauri/commands'
  import { reasoningLevelLabel } from '../ai/reasoning'
  import Icon from './Icon.svelte'

  /**
   * Reasoning-effort control (GWEN-458), separate from `ComposerModelMenu`
   * because the available levels are per-model, not a fixed cross-provider
   * set — Anthropic Opus offers low/medium/high/xhigh/max, Grok offers
   * none/low/medium/high, DeepSeek/GLM collapse to effectively binary, and
   * plenty of models (Mistral, Kimi's always-thinking code variant) support no
   * reasoning control at all. Hidden entirely when the active model's catalog
   * entry has `reasoning.supported === false`.
   */
  let { placement = 'up' }: { placement?: 'up' | 'down' } = $props()

  let open = $state(false)
  let activeIndex = $state(-1)
  let rootEl = $state<HTMLDivElement | null>(null)
  let catalog = $state<ModelEntry[]>([])

  $effect(() => {
    aiModelCatalog()
      .then((entries) => (catalog = entries))
      .catch(() => (catalog = []))
  })

  const activeEntry = $derived(catalog.find((m) => m.id === $aiChat.activeModel))
  const supported = $derived(activeEntry?.reasoning.supported ?? false)
  const levels = $derived(activeEntry?.reasoning.levels ?? [])

  // Keep the stored level valid for whichever model is active: fall back to
  // the model's default (or its first level) the moment the current value
  // isn't one of this model's levels — e.g. right after switching models.
  $effect(() => {
    if (!supported || levels.length === 0) return
    if (levels.includes($aiChat.reasoningLevel)) return
    const fallback = activeEntry?.reasoning.default ?? levels[0]
    setReasoningLevel(fallback)
  })

  function close() {
    open = false
    activeIndex = -1
  }
  function toggle() {
    if (open) close()
    else {
      open = true
      activeIndex = levels.indexOf($aiChat.reasoningLevel)
    }
  }
  function select(level: string) {
    setReasoningLevel(level)
    close()
  }

  function onKeydown(e: KeyboardEvent) {
    if (!supported) return
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        if (!open) toggle()
        else if (levels.length) activeIndex = (activeIndex + 1 + levels.length) % levels.length
        break
      case 'ArrowUp':
        e.preventDefault()
        if (!open) toggle()
        else if (levels.length) activeIndex = (activeIndex - 1 + levels.length) % levels.length
        break
      case 'Enter':
      case ' ':
        e.preventDefault()
        if (open && activeIndex >= 0 && levels[activeIndex]) select(levels[activeIndex])
        else toggle()
        break
      case 'Escape':
        if (open) {
          e.preventDefault()
          e.stopPropagation()
          close()
        }
        break
      case 'Tab':
        if (open) close()
        break
    }
  }

  function onWindowPointerDown(e: PointerEvent) {
    if (open && rootEl && !rootEl.contains(e.target as Node)) close()
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} />

{#if supported}
  <div class="rm" bind:this={rootEl}>
    <button
      type="button"
      class="rm-trigger"
      class:open
      aria-haspopup="menu"
      aria-expanded={open}
      aria-label="Reasoning effort"
      title="Reasoning effort"
      onclick={toggle}
      onkeydown={onKeydown}
    >
      <Icon name="brain" size={12} />
      <span class="rm-label">{reasoningLevelLabel($aiChat.reasoningLevel)}</span>
      <span class="rm-caret" class:flip={open}><Icon name="nav-arrow-down" size={12} /></span>
    </button>

    {#if open}
      <div class="rm-menu gw-anim-pop-bounce" class:up={placement === 'up'} role="listbox" aria-label="Reasoning effort">
        {#each levels as level, i (level)}
          <button
            type="button"
            role="option"
            aria-selected={level === $aiChat.reasoningLevel}
            class="rm-option"
            class:checked={level === $aiChat.reasoningLevel}
            class:active={activeIndex === i}
            onpointerenter={() => (activeIndex = i)}
            onclick={() => select(level)}
          >
            <span class="rm-check">{#if level === $aiChat.reasoningLevel}<Icon name="check" size={13} />{/if}</span>
            <span class="rm-option-label">{reasoningLevelLabel(level)}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .rm {
    position: relative;
    flex: 0 0 auto;
  }
  .rm-trigger {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding: 0 7px 0 8px;
    font-family: var(--font-sans);
    font-size: 11px;
    color: var(--ai-text-muted);
    background-color: transparent;
    border: none;
    border-radius: 999px;
    cursor: pointer;
    transition: color 0.12s ease, background-color 0.12s ease;
  }
  .rm-trigger:hover,
  .rm-trigger.open,
  .rm-trigger:focus-visible {
    outline: none;
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
  }
  .rm-label {
    white-space: nowrap;
  }
  .rm-caret {
    display: inline-flex;
    flex-shrink: 0;
    transition: transform 0.14s ease;
  }
  .rm-caret.flip {
    transform: rotate(180deg);
  }
  .rm-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 30;
    min-width: 140px;
    width: max-content;
    padding: 6px;
    background-color: var(--ai-bg-surface);
    border: none;
    border-radius: 18px;
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.45));
    transform-origin: top left;
  }
  .rm-menu.up {
    top: auto;
    bottom: calc(100% + 4px);
    transform-origin: bottom left;
  }
  .rm-option {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 7px 9px;
    font-family: var(--font-sans);
    font-size: 12px;
    text-align: left;
    color: var(--ai-text-primary);
    background: transparent;
    border: none;
    border-radius: 12px;
    cursor: pointer;
    transition: background-color 0.12s ease;
  }
  .rm-option.active {
    background-color: var(--ai-bg-hover);
  }
  .rm-option.checked {
    color: var(--ai-primary-light);
  }
  .rm-check {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    flex-shrink: 0;
    color: var(--ai-primary-light);
  }
  .rm-option-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
