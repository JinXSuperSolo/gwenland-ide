<script lang="ts">
  import { aiReviewDiff, gitDiffFile } from '../tauri/commands'
  import { renderMarkdown } from '../preview/markdown'
  import Icon from './Icon.svelte'

  // GWEN-330: read-only unified-diff viewer for one file. Rendered in a normal
  // editor tab; closing it touches no git state. Added lines green, removed red,
  // with old/new line numbers in the gutter.
  let { root, path, untracked }: { root: string; path: string; untracked: boolean } = $props()

  interface Row {
    kind: 'add' | 'del' | 'ctx' | 'hunk' | 'meta'
    text: string
    oldNo: number | null
    newNo: number | null
  }

  let rows = $state<Row[]>([])
  let loading = $state(true)
  let error = $state<string | null>(null)
  let reviewOpen = $state(false)
  let reviewLoading = $state(false)
  let reviewError = $state<string | null>(null)
  let reviewText = $state('')
  let reviewHtml = $state('')
  let reviewNonce = 0

  function parse(diff: string): Row[] {
    const out: Row[] = []
    let oldNo = 0
    let newNo = 0
    for (const line of diff.split('\n')) {
      if (line.startsWith('@@')) {
        const m = /@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line)
        if (m) {
          oldNo = parseInt(m[1], 10)
          newNo = parseInt(m[2], 10)
        }
        out.push({ kind: 'hunk', text: line, oldNo: null, newNo: null })
      } else if (
        line.startsWith('diff ') ||
        line.startsWith('index ') ||
        line.startsWith('--- ') ||
        line.startsWith('+++ ') ||
        line.startsWith('new file') ||
        line.startsWith('deleted file') ||
        line.startsWith('similarity ') ||
        line.startsWith('rename ')
      ) {
        out.push({ kind: 'meta', text: line, oldNo: null, newNo: null })
      } else if (line.startsWith('+')) {
        out.push({ kind: 'add', text: line.slice(1), oldNo: null, newNo: newNo++ })
      } else if (line.startsWith('-')) {
        out.push({ kind: 'del', text: line.slice(1), oldNo: oldNo++, newNo: null })
      } else if (line.startsWith('\\')) {
        out.push({ kind: 'meta', text: line, oldNo: null, newNo: null })
      } else {
        out.push({ kind: 'ctx', text: line.slice(1), oldNo: oldNo++, newNo: newNo++ })
      }
    }
    return out
  }

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

    gitDiffFile(root, path, untracked)
      .then((diff) => {
        if (currentRoot !== root || currentPath !== path || currentUntracked !== untracked) return
        rows = diff.trim() ? parse(diff) : []
      })
      .catch((e) => {
        if (currentRoot === root && currentPath === path && currentUntracked === untracked) {
          error = String(e)
        }
      })
      .finally(() => {
        if (currentRoot === root && currentPath === path && currentUntracked === untracked) {
          loading = false
        }
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
    if (loading || reviewLoading || rows.length === 0) return
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
      disabled={loading || reviewLoading || !!error || rows.length === 0}
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
    {:else if rows.length === 0}
      <div class="diff-info">No changes to display.</div>
    {:else}
      <div class="diff-grid" role="table">
        {#each rows as row}
          <div class="diff-row {row.kind}" role="row">
            <span class="ln old">{row.oldNo ?? ''}</span>
            <span class="ln new">{row.newNo ?? ''}</span>
            <span class="sign">
              {row.kind === 'add' ? '+' : row.kind === 'del' ? '-' : ''}
            </span>
            <span class="code">{row.text}</span>
          </div>
        {/each}
      </div>
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

  .diff-viewer {
    flex: 1;
    min-height: 0;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.5;
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

  .diff-grid {
    display: table;
    width: 100%;
    border-collapse: collapse;
  }

  .diff-row {
    display: table-row;
    white-space: pre;
  }

  .diff-row > span {
    display: table-cell;
    vertical-align: top;
  }

  .ln {
    width: 1%;
    min-width: 38px;
    padding: 0 8px;
    text-align: right;
    color: var(--muted-foreground);
    opacity: 0.6;
    user-select: none;
    border-right: 1px solid var(--border);
  }

  .sign {
    width: 14px;
    text-align: center;
    user-select: none;
    color: var(--muted-foreground);
  }

  .code {
    padding-right: 16px;
    width: 100%;
  }

  .diff-row.add {
    background-color: rgba(40, 167, 69, 0.14);
  }

  .diff-row.add .sign {
    color: #5fb572;
  }

  .diff-row.del {
    background-color: rgba(220, 53, 69, 0.14);
  }

  .diff-row.del .sign {
    color: #e0707c;
  }

  .diff-row.hunk {
    background-color: var(--secondary);
    color: var(--primary);
  }

  .diff-row.hunk .code {
    color: var(--primary);
  }

  .diff-row.meta {
    color: var(--muted-foreground);
    opacity: 0.7;
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
