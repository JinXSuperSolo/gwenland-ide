<script lang="ts">
  import { aiChat } from '../stores/ai-chat'
  import { setProvider, setModel } from '../ai/ai-chat-setup'
  import { aiModelCatalog, type ModelEntry, type ModelProvider } from '../tauri/commands'
  import Icon from './Icon.svelte'
  import ProviderIcon from './ProviderIcon.svelte'

  /**
   * Model picker (GWEN-456): one pill showing the active model that opens a
   * single flat, scrollable list of every model across all providers (from the
   * static `ai_model_catalog` registry) — brand icon, name, and a
   * "context window · $/1M" subtitle per row, no per-provider grouping.
   *
   * Picking a row whose provider differs from the active one calls
   * `setProvider` first (fire-and-forget — it persists + refreshes key status
   * in the background) then `setModel` with the catalog id directly; we don't
   * wait on `ai_list_models` to repopulate `$aiChat.models` since the catalog
   * id is what the adapter expects regardless.
   *
   * Reasoning effort now lives in the separate `ReasoningMenu` (GWEN-458):
   * levels vary per model (Anthropic's low/medium/high/xhigh/max vs. Grok's
   * none/low/medium/high vs. models with no reasoning at all), so a single
   * fixed Low/Medium/High control doesn't fit every provider.
   */
  let { placement = 'up' }: { placement?: 'up' | 'down' } = $props()

  // Native provider ids (as used by `setProvider`/engine settings) mapped to
  // the model catalog's `ModelProvider` key.
  const NATIVE_PROVIDER_CATALOG_ID: Record<string, ModelProvider> = {
    anthropic: 'anthropic',
    openai: 'open_ai',
    gemini: 'google',
  }

  let open = $state(false)
  let activeIndex = $state(-1)
  let rootEl = $state<HTMLDivElement | null>(null)
  let listEl = $state<HTMLDivElement | null>(null)
  let catalog = $state<ModelEntry[]>([])

  $effect(() => {
    aiModelCatalog()
      .then((entries) => (catalog = entries))
      .catch(() => (catalog = []))
  })

  function providerIdFor(entry: ModelEntry): string {
    return (
      Object.entries(NATIVE_PROVIDER_CATALOG_ID).find(([, cid]) => cid === entry.provider)?.[0] ??
      entry.provider
    )
  }
  function contextLabel(entry: ModelEntry): string {
    const tokens = entry.context_window
    const label = tokens % 1_000_000 === 0 ? `${tokens / 1_000_000}M` : `${Math.round(tokens / 1000)}K`
    return `${label} context window`
  }
  function priceLabel(entry: ModelEntry): string {
    return `$${entry.pricing.input_per_m}/$${entry.pricing.output_per_m} per 1M`
  }

  const activeEntry = $derived(catalog.find((m) => m.id === $aiChat.activeModel))
  const triggerLabel = $derived(
    activeEntry?.name ||
      $aiChat.models?.find((m) => m.id === $aiChat.activeModel)?.display_name ||
      $aiChat.activeModel ||
      'Model'
  )

  $effect(() => {
    const idx = activeIndex
    if (!listEl) return
    const row = listEl.children[idx] as HTMLElement | undefined
    row?.scrollIntoView({ block: 'nearest' })
  })

  function openMenu() {
    open = true
    activeIndex = catalog.findIndex((m) => m.id === $aiChat.activeModel)
  }
  function close() {
    open = false
    activeIndex = -1
  }
  function toggle() {
    if (open) close()
    else openMenu()
  }

  function select(entry: ModelEntry) {
    const providerId = providerIdFor(entry)
    if (providerId !== $aiChat.activeProvider) void setProvider(providerId)
    void setModel(entry.id)
    close()
  }

  function onKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        if (!open) openMenu()
        else if (catalog.length) activeIndex = (activeIndex + 1 + catalog.length) % catalog.length
        break
      case 'ArrowUp':
        e.preventDefault()
        if (!open) openMenu()
        else if (catalog.length) activeIndex = (activeIndex - 1 + catalog.length) % catalog.length
        break
      case 'Enter':
      case ' ':
        e.preventDefault()
        if (open && activeIndex >= 0 && catalog[activeIndex]) select(catalog[activeIndex])
        else openMenu()
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

<div class="cm" bind:this={rootEl}>
  <button
    type="button"
    class="cm-trigger"
    class:open
    aria-haspopup="menu"
    aria-expanded={open}
    aria-label="Model"
    title="Model"
    onclick={toggle}
    onkeydown={onKeydown}
  >
    {#if activeEntry}<ProviderIcon provider={activeEntry.provider} size={12} />{/if}
    <span class="cm-label">{triggerLabel}</span>
    <span class="cm-caret" class:flip={open}><Icon name="nav-arrow-down" size={12} /></span>
  </button>

  {#if open}
    <div
      class="cm-menu gw-anim-pop-bounce"
      class:up={placement === 'up'}
      role="listbox"
      aria-label="Model"
      bind:this={listEl}
    >
      {#each catalog as entry, i (entry.id)}
        <button
          type="button"
          role="option"
          aria-selected={entry.id === $aiChat.activeModel}
          class="cm-option"
          class:checked={entry.id === $aiChat.activeModel}
          class:active={activeIndex === i}
          onpointerenter={() => (activeIndex = i)}
          onclick={() => select(entry)}
        >
          <span class="cm-icon"><ProviderIcon provider={entry.provider} size={15} /></span>
          <span class="cm-option-text">
            <span class="cm-option-label">{entry.name}</span>
            <span class="cm-option-meta">{contextLabel(entry)} · {priceLabel(entry)}</span>
          </span>
          <span class="cm-check">{#if entry.id === $aiChat.activeModel}<Icon name="check" size={13} />{/if}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .cm {
    position: relative;
    flex: 0 1 auto;
    min-width: 0;
    max-width: 100%;
  }
  .cm-trigger {
    display: flex;
    align-items: center;
    gap: 5px;
    max-width: 100%;
    height: 24px;
    padding: 0 7px 0 9px;
    font-family: var(--font-sans);
    font-size: 11px;
    color: var(--ai-text-muted);
    background-color: transparent;
    border: none;
    border-radius: 999px;
    cursor: pointer;
    transition: color 0.12s ease, background-color 0.12s ease;
  }
  .cm-trigger:hover,
  .cm-trigger.open,
  .cm-trigger:focus-visible {
    outline: none;
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
  }
  .cm-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cm-caret {
    display: inline-flex;
    flex-shrink: 0;
    transition: transform 0.14s ease;
  }
  .cm-caret.flip {
    transform: rotate(180deg);
  }
  .cm-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 30;
    min-width: 260px;
    width: max-content;
    max-width: 320px;
    max-height: 360px;
    overflow-y: auto;
    padding: 6px;
    background-color: var(--ai-bg-surface);
    border: none;
    border-radius: 20px;
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.45));
    transform-origin: top center;
    scrollbar-width: thin;
    scrollbar-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent)) transparent;
  }
  .cm-menu.up {
    top: auto;
    bottom: calc(100% + 4px);
    transform-origin: bottom center;
  }
  .cm-menu::-webkit-scrollbar {
    width: 4px;
  }
  .cm-menu::-webkit-scrollbar-thumb {
    background-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent));
    border-radius: 999px;
  }
  .cm-option {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 9px;
    font-family: var(--font-sans);
    font-size: 12px;
    text-align: left;
    color: var(--ai-text-primary);
    background: transparent;
    border: none;
    border-radius: 14px;
    cursor: pointer;
    transition: background-color 0.12s ease;
  }
  .cm-option.active {
    background-color: var(--ai-bg-hover);
  }
  .cm-option.checked .cm-option-label {
    color: var(--ai-primary-light);
  }
  .cm-icon {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--ai-text-muted);
  }
  .cm-option-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .cm-option-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .cm-option-meta {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10.5px;
    color: var(--ai-text-muted);
  }
  .cm-check {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    flex-shrink: 0;
    color: var(--ai-primary-light);
  }
</style>
