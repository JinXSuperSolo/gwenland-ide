<script lang="ts">
  import { assistantMode, MODE_META, ASSISTANT_MODES, type AssistantMode } from '../stores/assistant-mode'
  import { agentic } from '../stores/agentic'
  import { changeAgentTier } from '../agentic/agentic-setup'
  import type { AgentTier } from '../tauri/commands'
  import Icon, { type IconName } from './Icon.svelte'

  /**
   * Single composer "+" entry point consolidating what used to be several
   * separate controls: the context-attach menu (current file / current
   * selection / image upload), the assistant mode switch (AssistantModeMenu),
   * and the agent autonomy tier (AgentTierMenu). Frees the toolbar row down to
   * just [+] [effort] [model] [send].
   *
   * Clicking "+" always opens this dropdown (a single click can't both open a
   * menu and the OS file picker); picking "Upload Image" closes the menu and
   * forwards to the parent's file input the same way the old attach-menu did.
   *
   * Flat sections, no chevrons or cascading flyouts, no hover-shift animation
   * (reverted from an earlier cascading-submenu pass per feedback against a
   * Codex-style reference). The panel is sized to match the composer's own
   * width exactly (measured live off `surfaceEl`, not a fixed px guess) and
   * kept compact — tight row padding, no per-item hint subtext except where
   * genuinely needed (tier rows, which are terse one-liners).
   */
  let {
    isChatLike,
    isAgent,
    agentStreaming,
    attachDisabled,
    surfaceEl,
    onAttachFile,
    onAttachSelection,
    onUploadImage,
  }: {
    isChatLike: boolean
    isAgent: boolean
    agentStreaming: boolean
    attachDisabled: boolean
    /** The composer card element — the menu is sized to match its width. */
    surfaceEl: HTMLElement | null
    onAttachFile: () => void
    onAttachSelection: () => void
    onUploadImage: () => void
  } = $props()

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

  let open = $state(false)
  let wrapEl = $state<HTMLDivElement>()
  // Panel geometry, measured live rather than guessed: full composer width,
  // left offset to align flush with the composer's left edge, and a max
  // height clamped to the space above the toolbar so it never renders above
  // the top of the window.
  let panelWidth = $state(260)
  let panelLeft = $state(0)
  let maxHeight = $state(360)

  $effect(() => {
    if (!open || !wrapEl || !surfaceEl) return
    const wrapRect = wrapEl.getBoundingClientRect()
    const surfaceRect = surfaceEl.getBoundingClientRect()
    const margin = 12
    panelWidth = surfaceRect.width;
    panelLeft = surfaceRect.left - wrapRect.left;
    maxHeight = Math.max(160, wrapRect.top - margin)
  })

  function toggle() {
    if (agentStreaming) return
    open = !open
  }
  function close() {
    open = false
  }

  function pickAttachFile() {
    close()
    onAttachFile()
  }
  function pickAttachSelection() {
    close()
    onAttachSelection()
  }
  function pickUploadImage() {
    close()
    onUploadImage()
  }
  function pickMode(mode: AssistantMode) {
    assistantMode.set(mode)
    close()
  }
  function pickTier(tier: AgentTier) {
    close()
    if (tier !== $agentic.tier) void changeAgentTier(tier)
  }

  $effect(() => {
    if (!open) return
    function onPointerDown(e: PointerEvent) {
      if (wrapEl && !wrapEl.contains(e.target as Node)) close()
    }
    window.addEventListener('pointerdown', onPointerDown, true)
    return () => window.removeEventListener('pointerdown', onPointerDown, true)
  })
</script>

<div class="cam" bind:this={wrapEl}>
  <button
    type="button"
    class="cam-trigger"
    aria-haspopup="menu"
    aria-expanded={open}
    aria-label="Composer actions"
    title="Upload image, mode & tier"
    disabled={agentStreaming}
    onclick={toggle}
  >
    <Icon name="plus" size={15} />
  </button>

  {#if open}
    <div
      class="cam-menu gw-anim-pop-bounce"
      role="menu"
      tabindex="-1"
      style:width={`${panelWidth}px`}
      style:left={`${panelLeft}px`}
      style:max-height={`${maxHeight}px`}
      onkeydown={(e) => {
        if (e.key === 'Escape') close()
      }}
    >
      <div class="cam-section">Add</div>
      {#if isChatLike}
        <button
          type="button"
          role="menuitem"
          class="cam-item"
          disabled={attachDisabled}
          onclick={pickAttachFile}
        >
          <span class="cam-item-icon"><Icon name="page" size={14} /></span>
          <span class="cam-item-label">Current File</span>
        </button>
        <button
          type="button"
          role="menuitem"
          class="cam-item"
          disabled={attachDisabled}
          onclick={pickAttachSelection}
        >
          <span class="cam-item-icon"><Icon name="text" size={14} /></span>
          <span class="cam-item-label">Current Selection</span>
        </button>
      {/if}
      <button type="button" role="menuitem" class="cam-item" onclick={pickUploadImage}>
        <span class="cam-item-icon"><Icon name="media-image" size={14} /></span>
        <span class="cam-item-label">Upload Image</span>
      </button>

      <div class="cam-section">Mode</div>
      {#each ASSISTANT_MODES as mode (mode)}
        <button
          type="button"
          class="cam-item"
          class:active={mode === $assistantMode}
          role="menuitemradio"
          aria-checked={mode === $assistantMode}
          onclick={() => pickMode(mode)}
        >
          <span class="cam-item-icon"><Icon name={MODE_META[mode].icon as IconName} size={14} /></span>
          <span class="cam-item-label">{MODE_META[mode].label}</span>
          {#if mode === $assistantMode}
            <Icon name="check" size={13} />
          {/if}
        </button>
      {/each}

      {#if isAgent}
        <div class="cam-section">Approval Tier</div>
        {#each TIERS as tier (tier.id)}
          <button
            type="button"
            class="cam-item"
            class:active={tier.id === $agentic.tier}
            role="menuitemradio"
            aria-checked={tier.id === $agentic.tier}
            onclick={() => pickTier(tier.id)}
          >
            <span class="cam-item-icon"><Icon name={tier.icon} size={14} /></span>
            <span class="cam-item-text">
              <span class="cam-item-label">{tier.label}</span>
              <span class="cam-item-hint">{tier.hint}</span>
            </span>
            {#if tier.id === $agentic.tier}
              <Icon name="check" size={13} />
            {/if}
          </button>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .cam {
    position: relative;
    display: inline-flex;
    flex-shrink: 0;
  }
  .cam-trigger {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 999px;
    background: color-mix(in srgb, var(--ai-bg-surface) 82%, transparent);
    color: var(--ai-text-primary);
    cursor: pointer;
    transition: background-color 0.12s ease;
  }
  .cam-trigger:hover:not(:disabled) {
    background-color: var(--ai-bg-hover);
  }
  .cam-trigger:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  /* Sized live to match the composer's own width (see `panelWidth`/`panelLeft`
     in script) instead of a fixed px guess, per "must be same width as the
     Composer UI" feedback. Compact rows, no hover shift. */
  .cam-menu {
    position: absolute;
    bottom: 30px;
    z-index: 50;
    padding: 4px;
    background-color: var(--ai-bg-surface);
    border: none;
    border-radius: 14px;
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.45));
    display: flex;
    flex-direction: column;
    gap: 0;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent)) transparent;
  }
  .cam-menu::-webkit-scrollbar {
    width: 4px;
  }
  .cam-menu::-webkit-scrollbar-thumb {
    background-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent));
    border-radius: 999px;
  }
  .cam-section {
    padding: 6px 8px 2px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--ai-text-muted);
  }
  .cam-section:not(:first-child) {
    margin-top: 2px;
    padding-top: 6px;
    border-top: 1px solid color-mix(in srgb, var(--ai-text-muted) 14%, transparent);
  }
  .cam-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--ai-text-primary);
    text-align: left;
    cursor: pointer;
    transition: background-color 0.12s ease;
  }
  .cam-item:hover:not(:disabled) {
    background-color: var(--ai-bg-hover);
  }
  .cam-item:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .cam-item.active {
    background-color: color-mix(in srgb, var(--ai-primary) 16%, transparent);
  }
  .cam-item-icon {
    display: inline-flex;
    color: var(--ai-primary);
    flex-shrink: 0;
  }
  .cam-item-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font: 600 12px var(--font-sans);
  }
  .cam-item-text {
    display: flex;
    flex-direction: column;
    gap: 0;
    flex: 1;
    min-width: 0;
  }
  .cam-item-hint {
    font-size: 10.5px;
    color: var(--ai-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
