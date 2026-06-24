<script lang="ts">
  import { aiChat, setReasoningLevel, type ReasoningLevel } from '../stores/ai-chat'
  import { setProvider, setModel } from '../ai/ai-chat-setup'
  import { REASONING_LEVELS, isThinkingCapable } from '../ai/reasoning'
  import Icon from './Icon.svelte'

  /**
   * Combined composer control (Codex-style): one pill showing the active model
   * (and reasoning level when applicable) that opens a single sectioned popup
   * for Reasoning, Provider, and Model. Replaces the three separate dropdowns in
   * the composer toolbar. Reuses M4 behavior — `setProvider` / `setModel` /
   * `setReasoningLevel` — and stores no secrets.
   */
  let { placement = 'up' }: { placement?: 'up' | 'down' } = $props()

  const NATIVE_PROVIDERS = [
    { id: 'anthropic', label: 'Anthropic' },
    { id: 'openai', label: 'OpenAI' },
    { id: 'gemini', label: 'Google Gemini' },
  ]

  let open = $state(false)
  let activeIndex = $state(-1)
  let rootEl = $state<HTMLDivElement | null>(null)

  const providers = $derived([
    ...NATIVE_PROVIDERS,
    ...$aiChat.genericProviders.map((g) => ({ id: g.id, label: g.display_name || g.id })),
  ])
  const hasModelList = $derived(!!$aiChat.models && $aiChat.models.length > 0)
  const thinking = $derived(isThinkingCapable($aiChat.activeProvider, $aiChat.activeModel))

  const modelLabel = $derived(
    $aiChat.models?.find((m) => m.id === $aiChat.activeModel)?.display_name ||
      $aiChat.activeModel ||
      'Model'
  )
  const reasoningLabel = $derived(
    REASONING_LEVELS.find((l) => l.id === $aiChat.reasoningLevel)?.label ?? ''
  )
  const triggerLabel = $derived(thinking ? `${modelLabel} · ${reasoningLabel}` : modelLabel)

  // Flat list of keyboard-navigable rows, in render order: reasoning, provider,
  // model. The manual model input (no model list) is reachable by mouse/Tab.
  type NavItem =
    | { kind: 'reasoning'; id: ReasoningLevel }
    | { kind: 'provider'; id: string }
    | { kind: 'model'; id: string }
  const navItems = $derived<NavItem[]>([
    ...(thinking ? REASONING_LEVELS.map((l) => ({ kind: 'reasoning' as const, id: l.id })) : []),
    ...providers.map((p) => ({ kind: 'provider' as const, id: p.id })),
    ...(hasModelList ? ($aiChat.models ?? []).map((m) => ({ kind: 'model' as const, id: m.id })) : []),
  ])
  const reasoningBase = 0
  const providerBase = $derived(thinking ? REASONING_LEVELS.length : 0)
  const modelBase = $derived(providerBase + providers.length)

  function openMenu() {
    open = true
    activeIndex = -1
  }
  function close() {
    open = false
    activeIndex = -1
  }
  function toggle() {
    if (open) close()
    else openMenu()
  }

  function activate(item: NavItem) {
    if (item.kind === 'reasoning') {
      setReasoningLevel(item.id)
      close()
    } else if (item.kind === 'provider') {
      void setProvider(item.id)
      // Keep the menu open so the user can pick a model for the new provider.
    } else {
      void setModel(item.id)
      close()
    }
  }

  function onKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        if (!open) openMenu()
        else if (navItems.length) activeIndex = (activeIndex + 1 + navItems.length) % navItems.length
        break
      case 'ArrowUp':
        e.preventDefault()
        if (!open) openMenu()
        else if (navItems.length) activeIndex = (activeIndex - 1 + navItems.length) % navItems.length
        break
      case 'Enter':
      case ' ':
        e.preventDefault()
        if (open && activeIndex >= 0 && navItems[activeIndex]) activate(navItems[activeIndex])
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

  function onManualModel(e: Event) {
    void setModel((e.currentTarget as HTMLInputElement).value)
  }
  function onManualKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      ;(e.currentTarget as HTMLInputElement).blur()
      close()
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
    aria-label="Model and reasoning"
    title="Model and reasoning"
    onclick={toggle}
    onkeydown={onKeydown}
  >
    <span class="cm-label">{triggerLabel}</span>
    <span class="cm-caret" class:flip={open}><Icon name="nav-arrow-down" size={12} /></span>
  </button>

  {#if open}
    <div class="cm-menu gw-anim-pop" class:up={placement === 'up'} role="menu" aria-label="Model and reasoning">
      {#if thinking}
        <div class="cm-section">Reasoning</div>
        {#each REASONING_LEVELS as l, i}
          <button
            type="button"
            role="menuitemradio"
            aria-checked={l.id === $aiChat.reasoningLevel}
            class="cm-option"
            class:checked={l.id === $aiChat.reasoningLevel}
            class:active={activeIndex === reasoningBase + i}
            onpointerenter={() => (activeIndex = reasoningBase + i)}
            onclick={() => activate({ kind: 'reasoning', id: l.id })}
          >
            <span class="cm-check">{#if l.id === $aiChat.reasoningLevel}<Icon name="check" size={13} />{/if}</span>
            <span class="cm-option-label">{l.label}</span>
          </button>
        {/each}
      {/if}

      <div class="cm-section">Provider</div>
      {#each providers as p, i}
        <button
          type="button"
          role="menuitemradio"
          aria-checked={p.id === $aiChat.activeProvider}
          class="cm-option"
          class:checked={p.id === $aiChat.activeProvider}
          class:active={activeIndex === providerBase + i}
          onpointerenter={() => (activeIndex = providerBase + i)}
          onclick={() => activate({ kind: 'provider', id: p.id })}
        >
          <span class="cm-check">{#if p.id === $aiChat.activeProvider}<Icon name="check" size={13} />{/if}</span>
          <span class="cm-option-label">{p.label}</span>
        </button>
      {/each}

      <div class="cm-section">Model</div>
      {#if hasModelList}
        {#each $aiChat.models ?? [] as m, i}
          <button
            type="button"
            role="menuitemradio"
            aria-checked={m.id === $aiChat.activeModel}
            class="cm-option"
            class:checked={m.id === $aiChat.activeModel}
            class:active={activeIndex === modelBase + i}
            onpointerenter={() => (activeIndex = modelBase + i)}
            onclick={() => activate({ kind: 'model', id: m.id })}
          >
            <span class="cm-check">{#if m.id === $aiChat.activeModel}<Icon name="check" size={13} />{/if}</span>
            <span class="cm-option-label">{m.display_name || m.id}</span>
          </button>
        {/each}
      {:else}
        <input
          class="cm-model-input"
          placeholder="model id"
          value={$aiChat.activeModel}
          onchange={onManualModel}
          onkeydown={onManualKeydown}
          aria-label="Model id"
        />
      {/if}
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
    gap: 3px;
    max-width: 100%;
    height: 24px;
    padding: 0 7px 0 9px;
    font-family: var(--font-sans);
    font-size: 11px;
    color: var(--ai-text-muted);
    background-color: transparent;
    border: 1px solid transparent;
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
    min-width: 180px;
    width: max-content;
    max-width: 260px;
    max-height: 320px;
    overflow-y: auto;
    padding: 4px;
    background-color: var(--ai-bg-surface);
    border: 1px solid var(--ai-border-subtle);
    border-radius: 12px;
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
  .cm-section {
    padding: 6px 8px 3px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--ai-text-muted);
  }
  .cm-option {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 8px;
    font-family: var(--font-sans);
    font-size: 12px;
    text-align: left;
    color: var(--ai-text-primary);
    background: transparent;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .cm-option.active {
    background-color: var(--ai-bg-hover);
  }
  .cm-option.checked {
    color: var(--ai-primary-light);
  }
  .cm-check {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    flex-shrink: 0;
    color: var(--ai-primary-light);
  }
  .cm-option-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cm-model-input {
    width: calc(100% - 8px);
    margin: 2px 4px 4px;
    height: 26px;
    padding: 0 9px;
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
    border: 1px solid var(--ai-border-subtle);
    border-radius: 8px;
  }
  .cm-model-input:focus {
    outline: none;
    border-color: color-mix(in srgb, var(--ai-primary) 35%, transparent);
  }
  .cm-model-input::placeholder {
    color: var(--ai-text-muted);
  }
</style>
