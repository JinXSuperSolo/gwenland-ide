<script lang="ts">
  import type { Snippet } from 'svelte'
  import Icon from '../Icon.svelte'

  // Compact active-row + expandable timeline. `label` is the live status (e.g.
  // "Reading src/main.rs", "Waiting for approval…") shown next to a disclosure
  // caret; when `shimmer` it sweeps left→right (subtle, IDE-native think effect).
  // The slot holds the detailed activity timeline, collapsed by default. Header
  // height is fixed so toggling never shifts layout.
  let {
    label,
    shimmer = false,
    children,
  }: { label: string; shimmer?: boolean; children?: Snippet } = $props()

  let open = $state(false)
</script>

<div class="td">
  <button type="button" class="td-head" onclick={() => (open = !open)} aria-expanded={open}>
    <span class="caret" class:open><Icon name="nav-arrow-right" size={11} /></span>
    <span class="label" class:shimmer>{label}</span>
  </button>
  {#if open}
    <div class="td-body">{@render children?.()}</div>
  {/if}
</div>

<style>
  .td {
    margin: 2px 0;
    min-width: 0;
  }
  .td-head {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    min-width: 0;
    height: 22px;
    padding: 0;
    border: none;
    background: transparent;
    cursor: pointer;
    color: var(--ai-text-muted);
    text-align: left;
  }
  .caret {
    display: inline-flex;
    flex-shrink: 0;
    transition: transform 0.14s ease;
    opacity: 0.7;
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    font-weight: 600;
  }
  /* Left→right light sweep across the word, clipped to the text. */
  .label.shimmer {
    background: linear-gradient(
      100deg,
      color-mix(in srgb, var(--ai-text-muted) 85%, transparent) 35%,
      var(--ai-primary-light) 50%,
      color-mix(in srgb, var(--ai-text-muted) 85%, transparent) 65%
    );
    background-size: 220% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
    color: transparent;
    animation: td-shimmer 1.5s linear infinite;
  }
  @keyframes td-shimmer {
    0% {
      background-position: 180% 0;
    }
    100% {
      background-position: -80% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .label.shimmer {
      animation: none;
      color: var(--ai-primary-light);
      -webkit-text-fill-color: var(--ai-primary-light);
    }
  }
  .td-body {
    margin-top: 2px;
    padding-left: 4px;
    min-width: 0;
  }
</style>
