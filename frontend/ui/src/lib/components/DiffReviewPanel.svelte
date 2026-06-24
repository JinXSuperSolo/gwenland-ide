<script lang="ts">
  import { diffReview, reviewSummary, setActiveFile, cancelReview } from '../stores/diff-review'
  import Icon from './Icon.svelte'

  /**
   * AI-pane summary of the active diff proposal (Requirement 14.5-14.10). Lists
   * changed files with per-file progress; clicking a file opens it and shows its
   * editor overlay. Updates live as hunks are accepted/rejected.
   */

  function fileName(path: string | null): string {
    if (!path) return '(new file)'
    return path.split(/[\\/]/).filter(Boolean).pop() || path
  }

  function fileProgress(hunks: { status: string }[]): { resolved: number; total: number } {
    const total = hunks.length
    const resolved = hunks.filter((h) => h.status !== 'pending').length
    return { resolved, total }
  }
</script>

{#if $diffReview.active}
  <section class="review-panel" aria-label="Diff review">
    <header class="rp-header">
      <span class="rp-title"><Icon name="page-plus" size={13} /> Proposed changes</span>
      <button class="rp-close" title="Exit review" aria-label="Exit review" onclick={cancelReview}>
        <Icon name="xmark" size={14} />
      </button>
    </header>

    <div class="rp-summary">
      {$reviewSummary.accepted}/{$reviewSummary.hunks} hunks ·
      <span class="rp-add">+{$reviewSummary.added}</span>
      <span class="rp-rem">−{$reviewSummary.removed}</span>
      {#if $reviewSummary.failed > 0}<span class="rp-fail">· {$reviewSummary.failed} failed</span>{/if}
    </div>

    <ul class="rp-files">
      {#each $diffReview.files as file (file.id)}
        {@const prog = fileProgress(file.hunks)}
        <li>
          <button
            class="rp-file"
            class:active={file.id === $diffReview.activeFileId}
            onclick={() => setActiveFile(file.id)}
          >
            <Icon name="page" size={13} />
            <span class="rp-file-name">{fileName(file.newPath ?? file.oldPath)}</span>
            <span class="rp-file-prog">{prog.resolved}/{prog.total}</span>
          </button>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .review-panel {
    flex-shrink: 0;
    margin: 0 12px 8px;
    background-color: var(--ai-bg-surface);
    border: 1px solid var(--ai-border-subtle);
    border-radius: 12px;
    overflow: hidden;
  }
  .rp-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 8px 6px 12px;
  }
  .rp-title {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 700;
    color: var(--ai-text-primary);
  }
  .rp-close {
    display: inline-flex;
    border: none;
    background: transparent;
    color: var(--ai-text-muted);
    cursor: pointer;
    border-radius: 6px;
    padding: 2px;
  }
  .rp-close:hover {
    color: var(--ai-text-primary);
    background-color: var(--ai-bg-hover);
  }
  .rp-summary {
    padding: 0 12px 6px;
    font-size: 11px;
    color: var(--ai-text-muted);
  }
  .rp-add {
    color: #5fb572;
  }
  .rp-rem,
  .rp-fail {
    color: #e0707c;
  }
  .rp-files {
    list-style: none;
    margin: 0;
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .rp-file {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 8px;
    font-size: 12px;
    color: var(--ai-text-primary);
    background: transparent;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    text-align: left;
  }
  .rp-file:hover {
    background-color: var(--ai-bg-hover);
  }
  .rp-file.active {
    background-color: var(--ai-bg-hover);
    color: var(--ai-primary-light);
  }
  .rp-file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rp-file-prog {
    font-size: 10.5px;
    color: var(--ai-text-muted);
    flex-shrink: 0;
  }
</style>
