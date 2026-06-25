<script lang="ts">
  import {
    clearLocalHistory,
    closeLocalHistory,
    localHistory,
    restoreSelectedHistory,
    selectHistoryEntry,
  } from '../stores/local-history'

  type DiffRow = {
    n: number
    current: string
    selected: string
    changed: boolean
  }

  function formatBytes(size: number): string {
    if (size < 1024) return `${size} B`
    if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
    return `${(size / (1024 * 1024)).toFixed(1)} MB`
  }

  function diffRows(current: string, selected: string): DiffRow[] {
    const a = current.split('\n')
    const b = selected.split('\n')
    const len = Math.max(a.length, b.length)
    return Array.from({ length: len }, (_, i) => ({
      n: i + 1,
      current: a[i] ?? '',
      selected: b[i] ?? '',
      changed: (a[i] ?? '') !== (b[i] ?? ''),
    }))
  }

  const rows = $derived(diffRows($localHistory.currentContent, $localHistory.selectedContent))
</script>

{#if $localHistory.open}
  <aside class="history-panel" aria-label="Local History">
    <header class="history-header">
      <div>
        <h2>Local History</h2>
        <p title={$localHistory.filePath ?? ''}>{$localHistory.filePath}</p>
      </div>
      <button type="button" class="icon-btn" aria-label="Close Local History" onclick={closeLocalHistory}>×</button>
    </header>

    <div class="history-body">
      <nav class="timeline" aria-label="History entries">
        {#if $localHistory.loading && $localHistory.entries.length === 0}
          <div class="timeline-empty">Loading...</div>
        {:else if $localHistory.entries.length === 0}
          <div class="timeline-empty">No history yet.</div>
        {:else}
          {#each $localHistory.entries as entry (entry.timestamp)}
            <button
              type="button"
              class="entry"
              class:active={entry.timestamp === $localHistory.selectedTimestamp}
              onclick={() => selectHistoryEntry(entry.timestamp)}
            >
              <span class="entry-time">{entry.timestamp}</span>
              <span class="entry-meta">{entry.source} · {formatBytes(entry.size)}</span>
            </button>
          {/each}
        {/if}
      </nav>

      <section class="diff-pane" aria-label="Current file compared with selected history entry">
        <div class="diff-toolbar">
          <span>Current</span>
          <span>Selected Version</span>
          <button type="button" onclick={restoreSelectedHistory} disabled={!$localHistory.selectedTimestamp}>
            Restore
          </button>
          <button type="button" onclick={clearLocalHistory} disabled={$localHistory.entries.length === 0}>
            Clear
          </button>
        </div>
        {#if $localHistory.error}
          <div class="history-error">{$localHistory.error}</div>
        {/if}
        <div class="diff-grid">
          {#each rows as row (row.n)}
            <div class="line-no">{row.n}</div>
            <pre class:changed={row.changed}>{row.current || ' '}</pre>
            <pre class:changed={row.changed}>{row.selected || ' '}</pre>
          {/each}
        </div>
      </section>
    </div>
  </aside>
{/if}

<style>
  .history-panel {
    position: fixed;
    inset: 42px 16px 34px auto;
    z-index: 90;
    width: min(920px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--background);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-xl);
  }
  .history-header {
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 13px;
    font-weight: 700;
    color: var(--foreground);
  }
  p {
    max-width: 680px;
    margin: 2px 0 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .icon-btn {
    width: 24px;
    height: 24px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
  }
  .icon-btn:hover {
    background: var(--secondary);
    color: var(--foreground);
  }
  .history-body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
  }
  .timeline {
    overflow: auto;
    border-right: 1px solid var(--border);
    background: var(--card);
  }
  .entry {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 3px;
    padding: 9px 10px;
    border: none;
    border-bottom: 1px solid var(--border);
    background: transparent;
    color: var(--foreground);
    cursor: pointer;
    text-align: left;
  }
  .entry:hover,
  .entry.active {
    background: var(--secondary);
  }
  .entry-time {
    font-size: 11px;
    font-family: var(--font-mono);
  }
  .entry-meta,
  .timeline-empty {
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .timeline-empty {
    padding: 12px;
  }
  .diff-pane {
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .diff-toolbar {
    height: 36px;
    display: grid;
    grid-template-columns: 1fr 1fr auto auto;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .diff-toolbar button {
    height: 24px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--secondary);
    color: var(--foreground);
    font-size: 11px;
    cursor: pointer;
  }
  .diff-toolbar button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .history-error {
    padding: 8px 10px;
    color: var(--destructive);
    font-size: 12px;
    border-bottom: 1px solid var(--border);
  }
  .diff-grid {
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
    color: var(--foreground);
  }
  pre.changed {
    background: color-mix(in srgb, var(--primary) 14%, transparent);
  }
</style>
