<script lang="ts" module>
  export interface CustomDropdownItem {
    value: string
    label: string
    /** Optional inline SVG string (16×16 viewBox) shown before the label. */
    svgIcon?: string
    disabled?: boolean
  }
</script>

<script lang="ts">
  /**
   * General-purpose custom dropdown using GwenLand design tokens.
   *
   * Props:
   *   items      — array of { value, label, svgIcon?, disabled? }
   *   value      — currently-selected value
   *   onSelect   — called with the chosen value
   *   placeholder — text shown when value matches no item
   *   label      — accessible name for the trigger button (aria-label)
   *   compact    — smaller height/font for toolbar contexts
   *   placement  — 'down' (default) | 'up' for composer-style panels
   *
   * Keyboard: ArrowUp/Down navigate, Enter/Space select, Escape close.
   */
  let {
    items = [],
    value = '',
    onSelect,
    placeholder = 'Select…',
    label = 'Select',
    compact = false,
    placement = 'down',
  }: {
    items: CustomDropdownItem[]
    value: string
    onSelect: (value: string) => void
    placeholder?: string
    label?: string
    compact?: boolean
    placement?: 'down' | 'up'
  } = $props()

  let open = $state(false)
  let activeIndex = $state(-1)
  let rootEl = $state<HTMLDivElement | null>(null)

  const selected = $derived(items.find((i) => i.value === value) ?? null)
  const triggerLabel = $derived(selected?.label ?? placeholder)
  const triggerIcon = $derived(selected?.svgIcon ?? null)

  function firstEnabled(): number {
    return items.findIndex((i) => !i.disabled)
  }

  function openMenu() {
    open = true
    const idx = items.findIndex((i) => i.value === value)
    activeIndex = idx >= 0 && !items[idx]?.disabled ? idx : firstEnabled()
  }
  function closeMenu() {
    open = false
    activeIndex = -1
  }
  function toggle() {
    if (open) closeMenu()
    else openMenu()
  }

  function move(delta: number) {
    if (!open) { openMenu(); return }
    const n = items.length
    if (n === 0) return
    let i = activeIndex
    for (let step = 0; step < n; step++) {
      i = (i + delta + n) % n
      if (!items[i]?.disabled) { activeIndex = i; return }
    }
  }

  function choose(i: number) {
    const item = items[i]
    if (!item || item.disabled) return
    onSelect(item.value)
    closeMenu()
  }

  function onKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case 'ArrowDown': e.preventDefault(); move(1); break
      case 'ArrowUp':   e.preventDefault(); move(-1); break
      case 'Enter':
      case ' ':
        e.preventDefault()
        if (open && activeIndex >= 0) choose(activeIndex)
        else openMenu()
        break
      case 'Escape':
        if (open) { e.preventDefault(); e.stopPropagation(); closeMenu() }
        break
      case 'Tab':
        if (open) closeMenu()
        break
    }
  }

  function onWindowPointerDown(e: PointerEvent) {
    if (open && rootEl && !rootEl.contains(e.target as Node)) closeMenu()
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} />

<div class="cd" class:compact bind:this={rootEl}>
  <button
    type="button"
    class="cd-trigger"
    class:open
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={label}
    onclick={toggle}
    onkeydown={onKeydown}
  >
    {#if triggerIcon}
      <span class="cd-icon" aria-hidden="true">{@html triggerIcon}</span>
    {/if}
    <span class="cd-label">{triggerLabel}</span>
    <svg class="cd-caret" class:flip={open} width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
      <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
  </button>

  {#if open}
    <div
      class="cd-panel"
      class:up={placement === 'up'}
      role="listbox"
      aria-label={label}
    >
      {#each items as item, i (item.value)}
        <button
          type="button"
          role="option"
          aria-selected={item.value === value}
          class="cd-item"
          class:active={i === activeIndex}
          class:selected={item.value === value}
          disabled={item.disabled}
          onpointerenter={() => (activeIndex = i)}
          onclick={() => choose(i)}
        >
          {#if item.svgIcon}
            <span class="cd-item-icon" aria-hidden="true">{@html item.svgIcon}</span>
          {/if}
          <span class="cd-item-label">{item.label}</span>
          {#if item.value === value}
            <svg class="cd-check" width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
              <path d="M2 6L5 9L10 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          {/if}
        </button>
      {/each}
      {#if items.length === 0}
        <div class="cd-empty">{placeholder}</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  /* ── GwenLand design tokens ── */
  .cd {
    --cd-bg:          #1f1e1e;
    --cd-primary:     #e18445;
    --cd-hover:       rgba(225, 132, 69, 0.10);
    --cd-active:      rgba(225, 132, 69, 0.18);
    --cd-border:      rgba(225, 132, 69, 0.15);
    --cd-radius:      0.5rem;
    --cd-font:        'Inter', var(--font-sans, sans-serif);
    --cd-text:        rgba(255, 255, 255, 0.88);
    --cd-text-muted:  rgba(255, 255, 255, 0.45);
    --cd-shadow:      0 8px 24px rgba(0, 0, 0, 0.4);

    position: relative;
    display: inline-flex;
    flex-direction: column;
    min-width: 0;
    font-family: var(--cd-font);
  }

  /* ── Trigger ── */
  .cd-trigger {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 10px;
    background: var(--cd-bg);
    border: 1px solid var(--cd-border);
    border-radius: var(--cd-radius);
    color: var(--cd-text);
    font-family: var(--cd-font);
    font-size: 13px;
    cursor: pointer;
    transition: border-color 0.12s ease, background-color 0.12s ease;
    white-space: nowrap;
    min-width: 0;
    width: 100%;
    text-align: left;
  }
  .cd.compact .cd-trigger {
    height: 26px;
    font-size: 12px;
    padding: 0 8px;
  }
  .cd-trigger:hover,
  .cd-trigger.open,
  .cd-trigger:focus-visible {
    border-color: var(--cd-primary);
    outline: none;
  }
  .cd-trigger:focus-visible {
    box-shadow: 0 0 0 2px rgba(225, 132, 69, 0.25);
  }

  .cd-icon {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
    color: var(--cd-primary);
  }
  .cd-icon :global(svg) {
    width: 14px;
    height: 14px;
  }
  .cd-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cd-caret {
    flex-shrink: 0;
    color: var(--cd-text-muted);
    transition: transform 0.14s ease;
  }
  .cd-caret.flip {
    transform: rotate(180deg);
  }

  /* ── Panel ── */
  .cd-panel {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 200;
    min-width: 100%;
    width: max-content;
    max-width: 280px;
    max-height: 260px;
    overflow-y: auto;
    padding: 4px;
    background: var(--cd-bg);
    border: 1px solid var(--cd-border);
    border-radius: var(--cd-radius);
    box-shadow: var(--cd-shadow);
    scrollbar-width: thin;
    scrollbar-color: var(--cd-border) transparent;
    animation: cd-open 0.1s ease-out;
    transform-origin: top center;
  }
  .cd-panel.up {
    top: auto;
    bottom: calc(100% + 4px);
    transform-origin: bottom center;
  }
  @keyframes cd-open {
    from { opacity: 0; transform: scaleY(0.95); }
    to   { opacity: 1; transform: scaleY(1); }
  }
  .cd-panel::-webkit-scrollbar { width: 4px; }
  .cd-panel::-webkit-scrollbar-thumb {
    background: var(--cd-border);
    border-radius: 999px;
  }

  /* ── Items ── */
  .cd-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 32px;
    padding: 0 8px;
    background: transparent;
    border: none;
    border-radius: calc(var(--cd-radius) - 2px);
    color: var(--cd-text);
    font-family: var(--cd-font);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    white-space: nowrap;
    transition: background-color 0.08s ease;
  }
  .cd.compact .cd-item {
    height: 28px;
    font-size: 12px;
  }
  .cd-item:hover:not(:disabled),
  .cd-item.active:not(:disabled) {
    background: var(--cd-hover);
  }
  .cd-item.selected {
    color: var(--cd-primary);
    background: var(--cd-active);
  }
  .cd-item:disabled {
    color: var(--cd-text-muted);
    cursor: default;
  }

  .cd-item-icon {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
    color: var(--cd-primary);
  }
  .cd-item-icon :global(svg) {
    width: 14px;
    height: 14px;
  }
  .cd-item-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cd-check {
    flex-shrink: 0;
    color: var(--cd-primary);
  }

  .cd-empty {
    padding: 8px;
    font-size: 12px;
    color: var(--cd-text-muted);
    text-align: center;
  }

  @media (prefers-reduced-motion: reduce) {
    .cd-panel { animation: none; }
  }
</style>
