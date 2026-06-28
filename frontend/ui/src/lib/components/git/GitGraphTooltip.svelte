<script lang="ts">
  import type { CommitNode } from '../../types/git'

  let {
    node,
    x,
    y,
  }: {
    node: CommitNode
    x: number
    y: number
  } = $props()

  const refs = $derived(node.refs.filter((ref) => ref !== 'HEAD').join(', '))
  const message = $derived(
    node.message.length > 60 ? `${node.message.slice(0, 57).trimEnd()}...` : node.message,
  )
</script>

<div class="git-graph-tooltip" style:left={`${x}px`} style:top={`${y}px`}>
  <div class="tip-head">
    <span class="hash">{node.shortHash}</span>
    {#if node.isHead}<span class="head">HEAD</span>{/if}
  </div>
  <div class="message">{message}</div>
  {#if refs}
    <div class="refs">{refs}</div>
  {/if}
  <div class="meta">
    <span>{node.author}</span>
    <span>{node.relativeDate || node.date}</span>
  </div>
</div>

<style>
  .git-graph-tooltip {
    position: absolute;
    z-index: 30;
    width: min(260px, calc(100% - 24px));
    max-width: 260px;
    padding: 8px 10px;
    pointer-events: none;
    border: 1px solid color-mix(in srgb, var(--primary) 24%, var(--border));
    border-radius: 6px;
    background: color-mix(in srgb, var(--popover) 94%, black);
    box-shadow: var(--shadow-md);
    color: var(--popover-foreground);
    font-family: var(--font-sans);
  }
  .tip-head,
  .meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
  }
  .hash {
    color: var(--primary);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
  }
  .head {
    padding: 1px 5px;
    border: 1px solid color-mix(in srgb, var(--primary) 35%, transparent);
    border-radius: 4px;
    color: var(--primary);
    font-size: 9px;
    font-weight: 700;
  }
  .message {
    margin-top: 5px;
    color: var(--foreground);
    font-size: 12px;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }
  .refs {
    margin-top: 5px;
    color: #7c9eff;
    font-family: var(--font-mono);
    font-size: 10.5px;
    line-height: 1.3;
    overflow-wrap: anywhere;
  }
  .meta {
    margin-top: 6px;
    color: var(--muted-foreground);
    font-size: 10.5px;
  }
  .meta span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
