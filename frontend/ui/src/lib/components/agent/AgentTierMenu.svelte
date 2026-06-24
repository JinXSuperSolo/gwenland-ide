<script lang="ts">
  import { agentic } from '../../stores/agentic'
  import { changeAgentTier } from '../../agentic/agentic-setup'
  import type { AgentTier } from '../../tauri/commands'
  import Icon, { type IconName } from '../Icon.svelte'

  // Autonomy-tier picker for the agent composer (M10 Wave 8). The current tier
  // is always visible; changing it is allowed only between iterations, so the
  // trigger is disabled while the agent is working (`disabled` prop).
  let { disabled = false }: { disabled?: boolean } = $props()

  const TIERS: { id: AgentTier; label: string; hint: string; icon: IconName }[] = [
    { id: 'ask', label: 'Ask', hint: 'Approve every edit & command', icon: 'clipboard-check' },
    {
      id: 'accept_for_me',
      label: 'Accept for Me',
      hint: 'Auto-approve small, safe, in-workspace changes',
      icon: 'check',
    },
    {
      id: 'full_control',
      label: 'Full Control',
      hint: 'Run autonomously; always stops at destructive steps',
      icon: 'magic-wand',
    },
  ]

  const current = $derived(TIERS.find((t) => t.id === $agentic.tier) ?? TIERS[0])

  let open = $state(false)
  let wrapEl = $state<HTMLDivElement>()

  function pick(tier: AgentTier) {
    open = false
    if (tier !== $agentic.tier) void changeAgentTier(tier)
  }

  $effect(() => {
    if (!open) return
    function onPointerDown(e: PointerEvent) {
      if (wrapEl && !wrapEl.contains(e.target as Node)) open = false
    }
    window.addEventListener('pointerdown', onPointerDown, true)
    return () => window.removeEventListener('pointerdown', onPointerDown, true)
  })
</script>

<div class="tier-wrap" bind:this={wrapEl}>
  <button
    type="button"
    class="tier-trigger"
    class:full={$agentic.tier === 'full_control'}
    aria-haspopup="menu"
    aria-expanded={open}
    title={`Agent tier: ${current.label}`}
    {disabled}
    onclick={() => (open = !open)}
  >
    <Icon name={current.icon} size={12} />
    <span class="tier-trigger-label">{current.label}</span>
    <Icon name="nav-arrow-down" size={11} />
  </button>

  {#if open}
    <div class="tier-menu" role="menu" tabindex="-1" onkeydown={(e) => { if (e.key === 'Escape') open = false }}>
      {#each TIERS as tier (tier.id)}
        <button
          type="button"
          class="tier-item"
          class:active={tier.id === $agentic.tier}
          role="menuitemradio"
          aria-checked={tier.id === $agentic.tier}
          onclick={() => pick(tier.id)}
        >
          <span class="tier-item-icon"><Icon name={tier.icon} size={15} /></span>
          <span class="tier-item-text">
            <span class="tier-item-label">{tier.label}</span>
            <span class="tier-item-hint">{tier.hint}</span>
          </span>
          {#if tier.id === $agentic.tier}
            <Icon name="check" size={14} />
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .tier-wrap {
    position: relative;
    display: inline-flex;
  }
  .tier-trigger {
    display: inline-flex;
    align-items: center;
    gap: 4px;
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
  .tier-trigger:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--ai-primary) 35%, transparent);
  }
  .tier-trigger:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .tier-trigger.full {
    border-color: color-mix(in srgb, var(--ai-primary) 45%, transparent);
    color: var(--ai-primary-light);
  }
  .tier-trigger-label {
    line-height: 1;
  }
  .tier-menu {
    position: absolute;
    bottom: 30px;
    left: 0;
    z-index: 50;
    min-width: 240px;
    padding: 4px;
    background-color: var(--ai-bg-surface);
    border: 1px solid var(--ai-border-subtle);
    border-radius: 12px;
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.45));
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .tier-item {
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
  .tier-item:hover {
    background-color: var(--ai-bg-hover);
  }
  .tier-item.active {
    background-color: color-mix(in srgb, var(--ai-primary) 14%, transparent);
  }
  .tier-item-icon {
    display: inline-flex;
    color: var(--ai-primary);
    flex-shrink: 0;
  }
  .tier-item-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }
  .tier-item-label {
    font: 600 12px var(--font-sans);
  }
  .tier-item-hint {
    font-size: 11px;
    color: var(--ai-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
