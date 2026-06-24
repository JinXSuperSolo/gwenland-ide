<script lang="ts">
  import type { ChatMessage } from '../stores/ai-chat'
  import { parseSegments } from '../ai/code-blocks'
  import { renderMarkdown } from '../ai/markdown'
  import AiCodeBlock from './AiCodeBlock.svelte'
  import ThinkingBlock from './ThinkingBlock.svelte'
  import MermaidGraph from './MermaidGraph.svelte'
  import Icon from './Icon.svelte'

  const GRAPH_LANGS = new Set(['mermaid', 'graph', 'flowchart'])
  const isGraphLang = (l: string) => GRAPH_LANGS.has((l || '').toLowerCase())
  import { startReview, diffReview } from '../stores/diff-review'
  import { projectRoot } from '../ai/ai-chat-setup'

  function reviewThis() {
    if (message.diff) startReview(message.id, message.diff.files, projectRoot())
  }

  /**
   * One chat message. Body is split into prose / fenced-code segments; code
   * segments render as AiCodeBlock (highlight + Copy + Insert). Tolerant of
   * incomplete fences while streaming.
   */
  let { message }: { message: ChatMessage } = $props()

  const isUser = $derived(message.role === 'user')
  const segments = $derived(parseSegments(message.content))

  // Per-file +added / −removed stats for the proposal card (Codex-style).
  let filesOpen = $state(true)
  const fileStats = $derived(
    (message.diff?.files ?? []).map((f) => {
      let added = 0
      let removed = 0
      for (const h of f.hunks)
        for (const l of h.lines) {
          if (l.kind === 'added') added++
          else if (l.kind === 'removed') removed++
        }
      const path = f.new_path ?? f.old_path ?? '(new file)'
      const parts = path.split(/[\\/]/)
      const base = parts.pop() || path
      return { path, base, dir: parts.join('/'), added, removed }
    })
  )
  const totalAdded = $derived(fileStats.reduce((n, f) => n + f.added, 0))
  const totalRemoved = $derived(fileStats.reduce((n, f) => n + f.removed, 0))
  const reviewing = $derived($diffReview.proposalId === message.id)
</script>

<div class="ai-msg" class:user={isUser} class:assistant={!isUser}>
  <div class="ai-msg-role">{isUser ? 'You' : 'Assistant'}</div>
  {#if message.attachments && message.attachments.length > 0}
    <div class="ai-msg-attachments">
      {#each message.attachments as att}
        <span class="chip">
          {att.type === 'file'
            ? att.path
            : att.type === 'selection'
              ? `selection · ${att.path}`
              : att.label}
        </span>
      {/each}
    </div>
  {/if}
  {#if message.images && message.images.length > 0}
    <div class="ai-msg-images">
      {#each message.images as img}
        <img class="ai-msg-image" src={`data:${img.mime};base64,${img.data}`} alt="attachment" />
      {/each}
    </div>
  {/if}
  {#if !isUser && message.thinking && (message.thinking.content || message.thinking.streaming)}
    <ThinkingBlock thinking={message.thinking} />
  {/if}
  <div class="ai-msg-content">
    {#each segments as seg}
      {#if seg.kind === 'code' && isGraphLang(seg.lang)}
        <MermaidGraph source={seg.content} />
      {:else if seg.kind === 'code'}
        <AiCodeBlock code={seg.content} lang={seg.lang} />
      {:else}
        <!-- eslint-disable-next-line svelte/no-at-html-tags — markdown.ts HTML-escapes before formatting -->
        <div class="ai-msg-text md">{@html renderMarkdown(seg.content)}</div>
      {/if}
    {/each}
    {#if message.streaming}<span class="ai-cursor" aria-hidden="true"></span>{/if}
  </div>

  {#if message.diff}
    <div class="diff-card" class:reviewing>
      <div class="diff-card-head">
        <span class="diff-card-badge"><Icon name="page-plus" size={14} /></span>
        <div class="diff-card-titles">
          <span class="diff-card-title">Proposed changes</span>
          <span class="diff-card-sub">
            {message.diff.fileCount} {message.diff.fileCount === 1 ? 'file' : 'files'}
            <span class="add">+{totalAdded}</span>
            <span class="rem">−{totalRemoved}</span>
          </span>
        </div>
        <button class="diff-card-review" onclick={reviewThis} disabled={reviewing}>
          {reviewing ? 'Reviewing…' : 'Review'}
        </button>
      </div>

      {#if filesOpen}
        <ul class="diff-card-files">
          {#each fileStats as f}
            <li class="diff-card-file">
              <Icon name="page" size={12} />
              <span class="diff-card-path">
                {#if f.dir}<span class="path-dir">{f.dir}/</span>{/if}<span class="path-name">{f.base}</span>
              </span>
              <span class="diff-card-counts">
                <span class="add">+{f.added}</span>
                <span class="rem">−{f.removed}</span>
              </span>
            </li>
          {/each}
        </ul>
      {/if}

      <button class="diff-card-toggle" onclick={() => (filesOpen = !filesOpen)}>
        {filesOpen ? 'Collapse files' : `Show ${message.diff.fileCount} ${message.diff.fileCount === 1 ? 'file' : 'files'}`}
        <span class="caret" class:open={filesOpen}><Icon name="nav-arrow-down" size={11} /></span>
      </button>
    </div>
  {:else if message.diffError}
    <div class="diff-notice warn" role="status">
      <Icon name="globe" size={13} />
      <span>This looked like a diff but couldn't be parsed for review.</span>
    </div>
  {/if}
</div>

<style>
  .ai-msg {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 12px;
    border-radius: 12px;
    font-size: 13px;
    line-height: 1.5;
  }
  .ai-msg.user {
    background-color: var(--ai-bg-surface, var(--secondary));
  }
  .ai-msg.assistant {
    background-color: transparent;
  }
  .ai-msg-role {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--ai-text-muted, var(--muted-foreground));
  }
  .ai-msg-content {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--ai-text-primary, var(--foreground));
  }
  .ai-msg-text {
    word-break: break-word;
  }
  /* Rendered Markdown (md.ts escapes HTML first). Tight, chat-friendly spacing. */
  .ai-msg-text.md :global(.md-p) {
    margin: 0 0 8px;
  }
  .ai-msg-text.md :global(.md-p:last-child) {
    margin-bottom: 0;
  }
  .ai-msg-text.md :global(.md-h) {
    margin: 12px 0 6px;
    font-weight: 700;
    line-height: 1.3;
  }
  .ai-msg-text.md :global(.md-h1) {
    font-size: 16px;
  }
  .ai-msg-text.md :global(.md-h2) {
    font-size: 15px;
  }
  .ai-msg-text.md :global(.md-h3) {
    font-size: 13.5px;
  }
  .ai-msg-text.md :global(.md-h4),
  .ai-msg-text.md :global(.md-h5),
  .ai-msg-text.md :global(.md-h6) {
    font-size: 13px;
    color: var(--ai-text-muted, var(--muted-foreground));
  }
  .ai-msg-text.md :global(.md-ul),
  .ai-msg-text.md :global(.md-ol) {
    margin: 4px 0 8px;
    padding-left: 20px;
  }
  .ai-msg-text.md :global(li) {
    margin: 2px 0;
  }
  .ai-msg-text.md :global(.md-code) {
    font-family: var(--font-mono);
    font-size: 0.92em;
    padding: 1px 5px;
    border-radius: 5px;
    background-color: var(--ai-bg-surface, var(--secondary));
  }
  .ai-msg-text.md :global(.md-quote) {
    margin: 4px 0 8px;
    padding: 2px 0 2px 10px;
    border-left: 2px solid var(--ai-border-subtle, var(--border));
    color: var(--ai-text-muted, var(--muted-foreground));
  }
  .ai-msg-text.md :global(strong) {
    font-weight: 700;
  }
  .ai-msg-text.md :global(a) {
    color: var(--ai-primary-light, var(--primary));
    text-decoration: underline;
  }
  .ai-msg-text.md :global(.md-table) {
    display: block;
    width: 100%;
    overflow-x: auto;
    border-collapse: collapse;
    margin: 6px 0 8px;
    font-size: 12px;
  }
  .ai-msg-text.md :global(.md-table th),
  .ai-msg-text.md :global(.md-table td) {
    padding: 4px 8px;
    border: 1px solid var(--ai-border-subtle, var(--border));
    text-align: left;
  }
  .ai-msg-text.md :global(.md-table th) {
    font-weight: 600;
    background-color: var(--ai-bg-surface, var(--secondary));
  }
  /* From-scratch LaTeX math (see ai/math.ts). */
  .ai-msg-text :global(.math-inline),
  .ai-msg-text :global(.math-block) {
    font-family: 'Cambria Math', 'Latin Modern Math', 'Times New Roman', serif;
    white-space: nowrap;
  }
  .ai-msg-text :global(.math-block) {
    display: block;
    text-align: center;
    margin: 8px 0;
    font-size: 1.05em;
  }
  .ai-msg-text :global(.math-frac) {
    display: inline-flex;
    flex-direction: column;
    vertical-align: middle;
    text-align: center;
    margin: 0 2px;
  }
  .ai-msg-text :global(.math-num) {
    border-bottom: 1px solid currentColor;
    padding: 0 4px;
  }
  .ai-msg-text :global(.math-den) {
    padding: 0 4px;
  }
  .ai-msg-text :global(.math-sqrt)::before {
    content: '√';
  }
  .ai-msg-text :global(.math-sqrt-body) {
    border-top: 1px solid currentColor;
    padding: 0 2px;
  }
  .ai-msg-text :global(.math-ss) {
    display: inline-flex;
    flex-direction: column;
    vertical-align: middle;
    font-size: 0.72em;
    line-height: 1;
  }
  .ai-msg-text :global(.math-ss sup),
  .ai-msg-text :global(.math-ss sub) {
    font-size: 1em;
    vertical-align: baseline;
    position: static;
  }
  .ai-msg-text :global(.math-func),
  .ai-msg-text :global(.math-text) {
    font-style: normal;
    font-family: var(--font-sans);
  }
  .ai-msg-text :global(.math-func) {
    padding-right: 2px;
  }
  /* Codex-style proposal card (Req 10.6 / 14): header summary + per-file list. */
  .diff-card {
    margin-top: 4px;
    background-color: var(--ai-bg-surface, var(--secondary));
    border: 1px solid var(--ai-border-subtle, var(--border));
    border-radius: 10px;
    overflow: hidden;
    font-family: var(--font-sans);
  }
  .diff-card-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 8px 10px;
  }
  .diff-card-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex-shrink: 0;
    border-radius: 7px;
    color: var(--ai-primary-light, var(--primary));
    background-color: var(--ai-thinking-bg, rgba(181, 105, 54, 0.06));
  }
  .diff-card-titles {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .diff-card-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--ai-text-primary, var(--foreground));
  }
  .diff-card-sub {
    font-size: 11px;
    color: var(--ai-text-muted, var(--muted-foreground));
  }
  .diff-card-review {
    flex-shrink: 0;
    font-size: 11.5px;
    font-weight: 600;
    padding: 4px 12px;
    border-radius: 999px;
    border: none;
    color: var(--ai-bg-base, #1f1e1e);
    background-color: var(--ai-primary, var(--primary));
    cursor: pointer;
    transition: opacity 0.12s ease;
  }
  .diff-card-review:hover:not(:disabled) {
    opacity: 0.88;
  }
  .diff-card-review:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .diff-card-files {
    list-style: none;
    margin: 0;
    padding: 2px 4px;
    border-top: 1px solid var(--ai-border-subtle, var(--border));
  }
  .diff-card-file {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    font-size: 11.5px;
    color: var(--ai-text-muted, var(--muted-foreground));
  }
  .diff-card-path {
    display: flex;
    min-width: 0;
    flex: 1;
  }
  .path-dir {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ai-text-muted, var(--muted-foreground));
    opacity: 0.7;
  }
  .path-name {
    flex-shrink: 0;
    white-space: nowrap;
    color: var(--ai-text-primary, var(--foreground));
  }
  .diff-card-counts {
    flex-shrink: 0;
    display: inline-flex;
    gap: 6px;
    font-variant-numeric: tabular-nums;
  }
  .diff-card .add {
    color: #5fb572;
  }
  .diff-card .rem {
    color: #e0707c;
  }
  .diff-card-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    width: 100%;
    padding: 5px;
    font-family: var(--font-sans);
    font-size: 11px;
    color: var(--ai-text-muted, var(--muted-foreground));
    background: transparent;
    border: none;
    border-top: 1px solid var(--ai-border-subtle, var(--border));
    cursor: pointer;
  }
  .diff-card-toggle:hover {
    color: var(--ai-text-primary, var(--foreground));
  }
  .diff-card-toggle .caret {
    display: inline-flex;
    transition: transform 0.14s ease;
  }
  .diff-card-toggle .caret.open {
    transform: rotate(180deg);
  }

  /* Non-destructive parse-failure notice (Req 10.6). */
  .diff-notice {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    margin-top: 2px;
    padding: 6px 10px;
    font-family: var(--font-sans);
    font-size: 11.5px;
    text-align: left;
    border-radius: 8px;
  }
  .diff-notice.warn {
    color: var(--ai-text-muted, #9b9b9b);
    background-color: var(--ai-bg-surface, #242120);
  }
  .ai-msg-attachments {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .ai-msg-images {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .ai-msg-image {
    max-width: 180px;
    max-height: 180px;
    border-radius: 8px;
    border: 1px solid var(--ai-border-subtle, var(--border));
    object-fit: contain;
  }
  .chip {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 999px;
    background-color: var(--muted);
    color: var(--muted-foreground);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Blinking streaming cursor. */
  .ai-cursor {
    display: inline-block;
    width: 6px;
    height: 1em;
    margin-left: 1px;
    vertical-align: text-bottom;
    background-color: var(--primary);
    animation: ai-blink 1s steps(2, start) infinite;
  }
  @keyframes ai-blink {
    to {
      visibility: hidden;
    }
  }
</style>
