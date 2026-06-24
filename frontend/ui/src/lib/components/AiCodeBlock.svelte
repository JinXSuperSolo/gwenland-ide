<script lang="ts">
  import { highlight } from '../ai/code-blocks'
  import { insertAtCursor } from '../editor/active-editor'
  import Icon from './Icon.svelte'

  /**
   * One fenced code block: language label + highlighted body + Copy and Insert
   * into Editor actions. Copy/Insert use the RAW code (no fences, no language
   * hint) — Requirement 13.9-13.11.
   */
  let { code, lang }: { code: string; lang: string } = $props()

  const html = $derived(highlight(code, lang))

  let copyState = $state<'idle' | 'ok' | 'err'>('idle')
  let insertState = $state<'idle' | 'ok' | 'err'>('idle')

  async function copy() {
    try {
      await navigator.clipboard.writeText(code)
      copyState = 'ok'
    } catch {
      copyState = 'err'
    }
    setTimeout(() => (copyState = 'idle'), 1500)
  }

  function insert() {
    insertState = insertAtCursor(code) ? 'ok' : 'err'
    setTimeout(() => (insertState = 'idle'), 1800)
  }
</script>

<div class="code-block">
  <div class="cb-header">
    <span class="cb-lang">{lang || 'text'}</span>
    <div class="cb-actions">
      <button
        class="cb-btn"
        class:ok={copyState === 'ok'}
        class:err={copyState === 'err'}
        title="Copy code"
        onclick={copy}
      >
        <Icon name={copyState === 'ok' ? 'clipboard-check' : 'copy'} size={12} />
        {copyState === 'ok' ? 'Copied' : copyState === 'err' ? 'Failed' : 'Copy'}
      </button>
      <button
        class="cb-btn"
        class:ok={insertState === 'ok'}
        class:err={insertState === 'err'}
        title="Insert at editor cursor"
        onclick={insert}
      >
        <Icon name="page-plus" size={12} />
        {insertState === 'ok' ? 'Inserted' : insertState === 'err' ? 'No editor' : 'Insert'}
      </button>
    </div>
  </div>
  <!-- eslint-disable-next-line svelte/no-at-html-tags — html is built from HTML-escaped source in code-blocks.ts -->
  <pre class="cb-body"><code>{@html html}</code></pre>
</div>

<style>
  .code-block {
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
    margin: 4px 0;
    background-color: var(--background);
  }
  .cb-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 6px 3px 10px;
    background-color: var(--secondary);
    border-bottom: 1px solid var(--border);
  }
  .cb-lang {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted-foreground);
  }
  .cb-actions {
    display: flex;
    gap: 4px;
  }
  .cb-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 20px;
    padding: 0 7px;
    font-size: 10.5px;
    color: var(--muted-foreground);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .cb-btn:hover {
    color: var(--foreground);
    background-color: var(--background);
  }
  .cb-btn.ok {
    color: #98c379;
  }
  .cb-btn.err {
    color: #e06c75;
  }
  .cb-body {
    margin: 0;
    padding: 10px;
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
    color: var(--foreground);
    white-space: pre;
    tab-size: 4;
  }
  .cb-body :global(.tok-comment) {
    color: var(--muted-foreground);
    font-style: italic;
  }
  .cb-body :global(.tok-string) {
    color: #98c379;
  }
  .cb-body :global(.tok-number) {
    color: #d19a66;
  }
  .cb-body :global(.tok-keyword) {
    color: #c678dd;
  }
</style>
