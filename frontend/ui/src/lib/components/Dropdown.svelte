<script lang="ts" module>
  /** Option shape for the shared dropdown primitive (design Q on dropdowns). */
  export interface DropdownOption {
    id: string
    label: string
    description?: string
    disabled?: boolean
  }
</script>

<script lang="ts">
  import Icon, { type IconName } from './Icon.svelte'

  /**
   * Shared custom dropdown primitive (Requirement 4). Replaces the native select
   * with a pill trigger + floating card using the single-choice menu pattern
   * (role="menu" / role="menuitemradio"). Keyboard: Arrow keys move the active
   * option, Enter selects, Escape closes; mouse selection and outside-click
   * close also work. Holds no secrets — it only renders the options it is given.
   */
  let {
    options,
    value,
    onSelect,
    label,
    placeholder = 'Select…',
    icon,
    disabled = false,
    compact = false,
    placement = 'down',
  }: {
    options: DropdownOption[]
    value: string
    onSelect: (id: string) => void
    label: string
    placeholder?: string
    icon?: IconName
    disabled?: boolean
    compact?: boolean
    placement?: 'down' | 'up'
  } = $props()

  let open = $state(false)
  let activeIndex = $state(-1)
  let rootEl = $state<HTMLDivElement | null>(null)

  const selected = $derived(options.find((o) => o.id === value) ?? null)
  const triggerLabel = $derived(selected?.label ?? placeholder)

  function firstEnabled(): number {
    return options.findIndex((o) => !o.disabled)
  }

  function openMenu() {
    open = true
    const idx = options.findIndex((o) => o.id === value)
    activeIndex = idx >= 0 && !options[idx]?.disabled ? idx : firstEnabled()
  }
  function closeMenu() {
    open = false
    activeIndex = -1
  }
  function toggle() {
    if (disabled) return
    if (open) closeMenu()
    else openMenu()
  }

  // Close the menu if the control becomes disabled while open.
  $effect(() => {
    if (disabled && open) closeMenu()
  })

  /** Move the active option, skipping disabled entries and wrapping around. */
  function move(delta: number) {
    if (!open) {
      openMenu()
      return
    }
    const n = options.length
    if (n === 0) return
    let i = activeIndex
    for (let step = 0; step < n; step++) {
      i = (i + delta + n) % n
      if (!options[i]?.disabled) {
        activeIndex = i
        return
      }
    }
  }

  function choose(i: number) {
    const opt = options[i]
    if (!opt || opt.disabled) return
    onSelect(opt.id)
    closeMenu()
  }

  function onKeydown(e: KeyboardEvent) {
    if (disabled) return
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        move(1)
        break
      case 'ArrowUp':
        e.preventDefault()
        move(-1)
        break
      case 'Enter':
      case ' ':
        e.preventDefault()
        if (open && activeIndex >= 0) choose(activeIndex)
        else openMenu()
        break
      case 'Escape':
        if (open) {
          e.preventDefault()
          e.stopPropagation()
          closeMenu()
        }
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

<div class="dd" class:compact bind:this={rootEl}>
  <button
    type="button"
    class="dd-trigger"
    class:open
    aria-haspopup="menu"
    aria-expanded={open}
    aria-label={label}
    title={label}
    {disabled}
    onclick={toggle}
    onkeydown={onKeydown}
  >
    {#if icon}<span class="dd-icon"><Icon name={icon} size={13} /></span>{/if}
    <span class="dd-trigger-label">{triggerLabel}</span>
    <span class="dd-caret" class:flip={open}><Icon name="nav-arrow-down" size={12} /></span>
  </button>

  {#if open}
    <div class="dd-menu gw-anim-pop" class:up={placement === 'up'} role="menu" aria-label={label}>
      {#each options as opt, i (opt.id)}
        <button
          type="button"
          role="menuitemradio"
          aria-checked={opt.id === value}
          class="dd-option"
          class:active={i === activeIndex}
          disabled={opt.disabled}
          onpointerenter={() => (activeIndex = i)}
          onclick={() => choose(i)}
        >
          <span class="dd-check">
            {#if opt.id === value}<Icon name="check" size={13} />{/if}
          </span>
          <span class="dd-option-body">
            <span class="dd-option-label">{opt.label}</span>
            {#if opt.description}<span class="dd-option-desc">{opt.description}</span>{/if}
          </span>
        </button>
      {/each}
      {#if options.length === 0}
        <div class="dd-empty">No options</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .dd {
    position: relative;
    flex: 1;
    min-width: 0;
  }
  /* Compact pills for the composer toolbar — size to content, not full width. */
  .dd.compact {
    flex: 0 1 auto;
    min-width: 0;
    max-width: 150px;
  }
  .dd-trigger {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    height: 26px;
    padding: 0 8px 0 10px;
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-surface);
    border: 1px solid transparent;
    border-radius: 999px;
    cursor: pointer;
    transition: border-color 0.12s ease, background-color 0.12s ease;
  }
  .dd.compact .dd-trigger {
    height: 24px;
    gap: 3px;
    padding: 0 7px 0 9px;
    font-size: 11px;
    background-color: transparent;
    color: var(--ai-text-muted);
  }
  .dd.compact .dd-trigger:hover:not(:disabled),
  .dd.compact .dd-trigger.open {
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
    border-color: transparent;
  }
  .dd-trigger:hover:not(:disabled),
  .dd-trigger.open,
  .dd-trigger:focus-visible {
    outline: none;
    border-color: var(--ai-border-subtle);
  }
  .dd-trigger:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .dd-icon {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--ai-text-muted);
  }
  .dd-trigger-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }
  .dd-caret {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--ai-text-muted);
    transition: transform 0.14s ease;
  }
  .dd-caret.flip {
    transform: rotate(180deg);
  }
  .dd-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 30;
    min-width: 100%;
    width: max-content;
    max-width: 260px;
    max-height: 280px;
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
  /* Open upward when the trigger sits at the bottom of the pane (composer). */
  .dd-menu.up {
    top: auto;
    bottom: calc(100% + 4px);
    transform-origin: bottom center;
  }
  .dd-menu::-webkit-scrollbar {
    width: 4px;
  }
  .dd-menu::-webkit-scrollbar-thumb {
    background-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent));
    border-radius: 999px;
  }
  .dd-option {
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
  .dd-option.active:not(:disabled) {
    background-color: var(--ai-bg-hover);
  }
  .dd-option[aria-checked='true'] {
    color: var(--ai-primary-light);
  }
  .dd-option:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .dd-check {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    flex-shrink: 0;
    color: var(--ai-primary-light);
  }
  .dd-option-body {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .dd-option-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dd-option-desc {
    font-size: 10.5px;
    color: var(--ai-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dd-empty {
    padding: 8px;
    font-size: 12px;
    color: var(--ai-text-muted);
    text-align: center;
  }
</style>
