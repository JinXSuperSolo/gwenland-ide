<script lang="ts">
  import { closeSimpleDiff, simpleDiff } from '../stores/simple-diff'

  function rows(left: string, right: string) {
    const a = left.split('\n')
    const b = right.split('\n')
    const len = Math.max(a.length, b.length)
    return Array.from({ length: len }, (_, i) => ({
      n: i + 1,
      left: a[i] ?? '',
      right: b[i] ?? '',
      changed: (a[i] ?? '') !== (b[i] ?? ''),
    }))
  }

  const diffRows = $derived(rows($simpleDiff.left, $simpleDiff.right))
</script>

{#if $simpleDiff.open}
  <aside class="diff-panel" aria-label="Diff Preview">
    <header>
      <div>
        <h2>{$simpleDiff.title}</h2>
      </div>
      <button type="button" aria-label="Close Diff Preview" onclick={closeSimpleDiff}>×</button>
    </header>
    <div class="labels">
      <span>{$simpleDiff.leftLabel}</span>
      <span>{$simpleDiff.rightLabel}</span>
    </div>
    <div class="grid">
      {#each diffRows as row (row.n)}
        <div class="line-no">{row.n}</div>
        <pre class:changed={row.changed}>{row.left || ' '}</pre>
        <pre class:changed={row.changed}>{row.right || ' '}</pre>
      {/each}
    </div>
  </aside>
{/if}

<style>
  .diff-panel {
    position: fixed;
    inset: 54px 22px 38px 22px;
    z-index: 92;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--background);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-xl);
  }
  header {
    height: 42px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 13px;
  }
  button {
    width: 24px;
    height: 24px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
  }
  button:hover {
    background: var(--secondary);
    color: var(--foreground);
  }
  .labels {
    height: 32px;
    display: grid;
    grid-template-columns: 48px 1fr 1fr;
    align-items: center;
    border-bottom: 1px solid var(--border);
    color: var(--muted-foreground);
    font-size: 11px;
  }
  .labels::before {
    content: '';
  }
  .grid {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr) minmax(0, 1fr);
    align-content: start;
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .line-no,
  pre {
    margin: 0;
    min-height: 20px;
    padding: 2px 8px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    white-space: pre;
  }
  .line-no {
    color: var(--muted-foreground);
    text-align: right;
    background: var(--card);
    user-select: none;
  }
  pre {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  pre.changed {
    background: color-mix(in srgb, var(--primary) 14%, transparent);
  }
</style>
