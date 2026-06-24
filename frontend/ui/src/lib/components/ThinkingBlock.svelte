<script lang="ts">
  import type { ThinkingState } from '../stores/ai-chat'
  import Icon from './Icon.svelte'

  /**
   * Collapsible reasoning trace shown before the final answer (Requirement 8).
   * While streaming it shows `Thinking…`, caps its height and auto-scrolls; when
   * complete it auto-collapses to `Thought for <duration>`. The user can expand
   * or collapse at any time (their choice overrides the auto state).
   */
  let { thinking }: { thinking: ThinkingState } = $props()

  // null = follow the store's auto-collapse; otherwise the user's explicit choice.
  let userToggled = $state<boolean | null>(null)
  const collapsed = $derived(userToggled ?? thinking.collapsed)

  let bodyEl = $state<HTMLDivElement | null>(null)

  const durationMs = $derived(
    thinking.startedAt != null && thinking.endedAt != null
      ? thinking.endedAt - thinking.startedAt
      : null
  )

  function fmtDuration(ms: number): string {
    const s = ms / 1000
    if (s < 60) return `${s < 10 ? s.toFixed(1) : Math.round(s)}s`
    const m = Math.floor(s / 60)
    return `${m}m ${Math.round(s % 60)}s`
  }

  // Auto-scroll the trace as it streams in (Req 8.3).
  $effect(() => {
    void thinking.content
    if (thinking.streaming && bodyEl) bodyEl.scrollTop = bodyEl.scrollHeight
  })

  function toggle() {
    userToggled = !collapsed
  }
</script>

<div class="thinking">
  <button class="th-header" onclick={toggle} aria-expanded={!collapsed}>
    <span class="th-caret" class:open={!collapsed}><Icon name="nav-arrow-right" size={12} /></span>
    {#if thinking.streaming}
      <span class="th-dot"></span>
      <span class="th-label">Thinking…</span>
    {:else if durationMs != null}
      <span class="th-label">Thought for</span>
      <span class="th-dur">{fmtDuration(durationMs)}</span>
    {:else}
      <span class="th-label">Reasoning</span>
    {/if}
  </button>

  {#if !collapsed}
    <div class="th-body" class:capped={thinking.streaming} bind:this={bodyEl}>{thinking.content}</div>
  {/if}
</div>

<style>
  .thinking {
    border-left: 2px solid var(--ai-primary);
    background-color: var(--ai-thinking-bg);
    border-radius: 0 8px 8px 0;
    overflow: hidden;
  }
  .th-header {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    padding: 6px 8px;
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ai-text-muted);
    background: transparent;
    border: none;
    cursor: pointer;
    text-align: left;
  }
  .th-header:hover {
    color: var(--ai-text-primary);
  }
  .th-caret {
    display: inline-flex;
    flex-shrink: 0;
    transition: transform 0.14s ease;
  }
  .th-caret.open {
    transform: rotate(90deg);
  }
  .th-label {
    font-weight: 500;
  }
  .th-dur {
    font-size: 11px;
    color: var(--ai-text-muted);
  }
  .th-dot {
    width: 6px;
    height: 6px;
    flex-shrink: 0;
    border-radius: 50%;
    background-color: var(--ai-primary-light);
    animation: th-pulse 1s ease-in-out infinite;
  }
  @keyframes th-pulse {
    0%,
    100% {
      opacity: 0.35;
    }
    50% {
      opacity: 1;
    }
  }
  .th-body {
    max-height: 320px;
    overflow-y: auto;
    padding: 0 10px 8px 12px;
    font-family: var(--font-sans);
    font-size: 12px;
    line-height: 1.5;
    color: var(--ai-text-muted);
    white-space: pre-wrap;
    word-break: break-word;
    scrollbar-width: thin;
    scrollbar-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent)) transparent;
  }
  /* Cap height while streaming (Req 8.3). */
  .th-body.capped {
    max-height: 200px;
  }
  .th-body::-webkit-scrollbar {
    width: 4px;
  }
  .th-body::-webkit-scrollbar-thumb {
    background-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent));
    border-radius: 999px;
  }
</style>
