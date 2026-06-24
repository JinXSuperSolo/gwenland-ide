<script lang="ts">
  import type { Snippet } from 'svelte'
  import Icon from './Icon.svelte'

  /**
   * Custom checkbox used across settings and the AI pane. A visually-hidden
   * native input keeps full keyboard/accessibility behavior (Space toggles,
   * focus ring) while a styled box renders the GwenLand look. `onCheck` reports
   * the new boolean so call sites never touch DOM events.
   */
  let {
    checked = false,
    onCheck,
    title,
    children,
  }: {
    checked?: boolean
    onCheck?: (checked: boolean) => void
    title?: string
    children?: Snippet
  } = $props()
</script>

<label class="gw-checkbox" {title}>
  <input
    type="checkbox"
    {checked}
    onchange={(e) => onCheck?.((e.currentTarget as HTMLInputElement).checked)}
  />
  <span class="gw-checkbox-box" class:checked aria-hidden="true">
    {#if checked}<Icon name="check" size={11} strokeWidth={2.4} />{/if}
  </span>
  {#if children}<span class="gw-checkbox-label">{@render children()}</span>{/if}
</label>

<style>
  .gw-checkbox {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  /* Hidden but still focusable/operable for accessibility. */
  .gw-checkbox input {
    position: absolute;
    width: 1px;
    height: 1px;
    opacity: 0;
    pointer-events: none;
  }
  .gw-checkbox-box {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    border: 1px solid var(--border);
    border-radius: 5px;
    background-color: var(--input);
    color: var(--primary-foreground);
    transition: background-color 0.12s ease, border-color 0.12s ease;
  }
  .gw-checkbox:hover .gw-checkbox-box {
    border-color: var(--primary);
  }
  .gw-checkbox-box.checked {
    background-color: var(--primary);
    border-color: var(--primary);
  }
  .gw-checkbox input:focus-visible + .gw-checkbox-box {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--primary) 35%, transparent);
  }
  .gw-checkbox-label {
    font-size: 12px;
    color: var(--muted-foreground);
  }
</style>
