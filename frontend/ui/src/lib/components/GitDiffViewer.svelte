<script lang="ts">
  import { aiReviewDiff, gitDiffFile, parseDiff, type DiffFile } from '../tauri/commands'
  import { renderMarkdown } from '../preview/markdown'
  import Icon from './Icon.svelte'
  import DiffView from './DiffView.svelte'

  // GWEN-330 / GWEN-459: read-only diff viewer for one file. Rendered in a
  // normal editor tab; closing it touches no git state. Parsing is done by the
  // engine (`parse_unified_diff` via `parseDiff`) and rendering is delegated to
  // the shared `DiffView` component (Unified/Split, token colors, line numbers).
  let { root, path, untracked }: { root: string; path: string; untracked: boolean } = $props()

  let files = $state<DiffFile[]>([])
  let loading = $state(true)
  let error = $state<string | null>(null)
  let reviewOpen = $state(false)
  let reviewLoading = $state(false)
  let reviewError = $state<string | null>(null)
  let reviewText = $state('')
  let reviewHtml = $state('')
  let reviewNonce = 0

  const hasChanges = $derived(files.some((f) => f.hunks.length > 0))

  $effect(() => {
    loading = true
    error = null
    reviewOpen = false
    reviewLoading = false
    reviewError = null
    reviewText = ''
    reviewHtml = ''
    const currentRoot = root
    const currentPath = path
    const currentUntracked = untracked
    const stale = () =>
      currentRoot !== root || currentPath !== path || currentUntracked !== untracked

    gitDiffFile(root, path, untracked)
      .then(async (diff) => {
        if (stale()) return
        files = diff.trim() ? await parseDiff(diff) : []
      })
      .catch((e) => {
        if (!stale()) error = String(e)
      })
      .finally(() => {
        if (!stale()) loading = false
      })
  })

  function escapeHtml(text: string): string {
    return text
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&#39;')
  }

  async function renderReview(text: string): Promise<string> {
    try {
      return await renderMarkdown(text)
    } catch {
      return `<pre>${escapeHtml(text)}</pre>`
    }
  }

  async function startReview(): Promise<void> {
    if (loading || reviewLoading || !hasChanges) return
    const nonce = ++reviewNonce
    reviewOpen = true
    reviewLoading = true
    reviewError = null
    reviewText = ''
    reviewHtml = ''
    try {
      const text = await aiReviewDiff(root, path, untracked)
      if (nonce !== reviewNonce) return
      reviewText = text
      reviewHtml = await renderReview(text)
    } catch (e) {
      if (nonce === reviewNonce) reviewError = String(e)
    } finally {
      if (nonce === reviewNonce) reviewLoading = false
    }
  }

  async function copyReview(): Promise<void> {
    if (!reviewText) return
    try {
      await navigator.clipboard.writeText(reviewText)
    } catch {
      // Clipboard access can be denied by the WebView permission model.
    }
  }
</script>

<div class="diff-shell">
  <header class="diff-header">
    <div class="diff-title" title={path}>
      <Icon name="git-branch" size={14} />
      <span>{path}</span>
      {#if untracked}
        <span class="diff-pill">Untracked</span>
      {/if}
    </div>
    <button
      type="button"
      class="review-btn"
      disabled={loading || reviewLoading || !!error || !hasChanges}
      aria-busy={reviewLoading}
      onclick={startReview}
    >
      {#if reviewLoading}
        <span class="review-spinner" aria-hidden="true"></span>
      {:else}
        <Icon name="sparks" size={14} />
      {/if}
      <span>{reviewLoading ? 'Reviewing' : 'Review with AI'}</span>
    </button>
  </header>

  <div class="diff-viewer">
    {#if loading}
      <div class="diff-info">Loading diff...</div>
    {:else if error}
      <div class="diff-info error">{error}</div>
    {:else if !hasChanges}
      <div class="diff-info">No changes to display.</div>
    {:else}
      <DiffView {files} />
    {/if}
  </div>

  {#if reviewOpen}
    <section class="review-drawer" aria-label="AI Diff Review">
      <div class="review-head">
        <div class="review-title">
          <Icon name="sparks" size={14} />
          <span>AI Review</span>
        </div>
        <div class="review-actions">
          {#if reviewText && !reviewLoading}
            <button type="button" class="review-icon-btn" aria-label="Copy review" onclick={copyReview}>
              <Icon name="copy" size={14} />
            </button>
          {/if}
          <button
            type="button"
            class="review-icon-btn"
            aria-label="Close review"
            onclick={() => (reviewOpen = false)}
          >
            <Icon name="xmark" size={14} />
          </button>
        </div>
      </div>

      {#if reviewLoading}
        <div class="review-state">
          <span class="review-spinner" aria-hidden="true"></span>
          <span>Reviewing diff...</span>
        </div>
      {:else if reviewError}
        <div class="review-state error">{reviewError}</div>
      {:else}
        <div class="review-content">{@html reviewHtml}</div>
      {/if}
    </section>
  {/if}
</div>

<style>
  .diff-shell {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background-color: var(--background);
    color: var(--foreground);
    overflow: hidden;
  }

  .diff-header {
    flex-shrink: 0;
    height: 38px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 0 10px 0 12px;
    border-bottom: 1px solid var(--border);
    background: var(--background);
  }

  .diff-title {
    min-width: 0;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .diff-title span:first-of-type {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .diff-pill {
    flex-shrink: 0;
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--primary) 16%, transparent);
    color: var(--primary);
    font-family: var(--font-sans);
    font-size: 10px;
    font-weight: 700;
  }

  .review-btn,
  .review-icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: var(--radius-sm);
    font-family: var(--font-sans);
    cursor: pointer;
    transition: color 0.12s ease, background-color 0.12s ease, opacity 0.12s ease;
  }

  .review-btn {
    flex-shrink: 0;
    height: 26px;
    gap: 6px;
    padding: 0 10px;
    background: var(--primary);
    color: var(--primary-foreground);
    font-size: 11px;
    font-weight: 700;
  }

  .review-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--primary) 82%, #ffffff);
  }

  .review-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* Diff rendering (grid/rows/colors) now lives in the shared DiffView
     component; this viewer only owns the surrounding chrome + AI review drawer. */
  .diff-viewer {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .diff-info {
    padding: 16px;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 13px;
  }

  .diff-info.error {
    color: var(--destructive);
  }

  .review-drawer {
    flex-shrink: 0;
    max-height: min(38vh, 360px);
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
    background: var(--card);
    box-shadow: 0 -10px 30px rgba(0, 0, 0, 0.2);
  }

  .review-head {
    flex-shrink: 0;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 0 8px 0 12px;
    border-bottom: 1px solid var(--border);
  }

  .review-title,
  .review-actions,
  .review-state {
    display: inline-flex;
    align-items: center;
  }

  .review-title {
    gap: 7px;
    min-width: 0;
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 700;
  }

  .review-actions {
    gap: 2px;
  }

  .review-icon-btn {
    width: 28px;
    height: 28px;
    background: transparent;
    color: var(--muted-foreground);
  }

  .review-icon-btn:hover {
    color: var(--foreground);
    background: var(--secondary);
  }

  .review-state {
    gap: 8px;
    padding: 14px 16px 18px;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 12px;
  }

  .review-state.error {
    color: var(--destructive);
  }

  .review-spinner {
    width: 12px;
    height: 12px;
    flex: 0 0 auto;
    border: 2px solid color-mix(in srgb, currentColor 28%, transparent);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .review-content {
    min-height: 0;
    overflow: auto;
    padding: 12px 16px 18px;
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 12.5px;
    line-height: 1.55;
  }

  .review-content :global(h1),
  .review-content :global(h2),
  .review-content :global(h3) {
    margin: 0 0 7px;
    font-size: 13px;
    line-height: 1.3;
  }

  .review-content :global(p),
  .review-content :global(ul),
  .review-content :global(ol),
  .review-content :global(blockquote),
  .review-content :global(pre) {
    margin: 0 0 9px;
  }

  .review-content :global(ul),
  .review-content :global(ol) {
    padding-left: 20px;
  }

  .review-content :global(pre) {
    overflow: auto;
    padding: 9px 10px;
    border-radius: var(--radius-sm);
    background: var(--background);
    font-family: var(--font-mono);
  }

  .review-content :global(code) {
    font-family: var(--font-mono);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
