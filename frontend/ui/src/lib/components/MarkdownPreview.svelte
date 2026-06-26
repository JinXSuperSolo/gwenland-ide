<script lang="ts">
  import { tick } from 'svelte'
  import { renderMarkdown } from '../preview/markdown'

  let { source }: { source: string } = $props()

  let articleEl = $state<HTMLElement | null>(null)
  let html = $state('')

  $effect(() => {
    const src = source
    void renderMarkdown(src).then((rendered) => {
      html = rendered
      // Inject KaTeX stylesheet lazily the first time math is rendered
      if (rendered.includes('katex')) {
        const id = 'katex-css'
        if (!document.getElementById(id)) {
          const link = document.createElement('link')
          link.id = id
          link.rel = 'stylesheet'
          link.href = new URL('katex/dist/katex.min.css', import.meta.url).href
          document.head.appendChild(link)
        }
      }
      void tick()
    })
  })
</script>

<article class="markdown-preview" bind:this={articleEl}>
  {@html html}
</article>

<style>
  .markdown-preview {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: auto;
    padding: 22px 28px;
    border-left: 1px solid var(--border);
    background: color-mix(in srgb, var(--background) 94%, var(--card));
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 14px;
    line-height: 1.65;
  }
  .markdown-preview :global(h1),
  .markdown-preview :global(h2),
  .markdown-preview :global(h3),
  .markdown-preview :global(h4),
  .markdown-preview :global(h5),
  .markdown-preview :global(h6) {
    line-height: 1.2;
    margin: 0 0 12px;
    color: var(--foreground);
  }
  .markdown-preview :global(h1) { font-size: 28px; }
  .markdown-preview :global(h2) { font-size: 21px; margin-top: 24px; }
  .markdown-preview :global(h3) { font-size: 17px; margin-top: 20px; }
  .markdown-preview :global(h4) { font-size: 15px; margin-top: 16px; }
  .markdown-preview :global(hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 20px 0;
  }
  .markdown-preview :global(blockquote) {
    border-left: 3px solid var(--primary);
    margin: 0 0 14px;
    padding: 6px 14px;
    color: var(--muted-foreground);
    background: var(--secondary);
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
  }
  .markdown-preview :global(blockquote p) { margin: 0; }
  .markdown-preview :global(p),
  .markdown-preview :global(ul),
  .markdown-preview :global(ol),
  .markdown-preview :global(table),
  .markdown-preview :global(pre) {
    margin: 0 0 14px;
  }
  .markdown-preview :global(ul),
  .markdown-preview :global(ol) {
    padding-left: 22px;
  }
  .markdown-preview :global(li) { margin-bottom: 4px; }
  .markdown-preview :global(li input[type='checkbox']) {
    margin-right: 6px;
    accent-color: var(--primary);
  }
  .markdown-preview :global(a) { color: var(--primary); }
  .markdown-preview :global(strong) { font-weight: 600; }
  .markdown-preview :global(del) { opacity: 0.6; }
  .markdown-preview :global(code) {
    font-family: var(--font-mono);
    font-size: 0.92em;
    padding: 1px 4px;
    border-radius: 3px;
    background: var(--secondary);
  }
  .markdown-preview :global(pre) {
    overflow: auto;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: #171615;
  }
  .markdown-preview :global(pre code) {
    padding: 0;
    background: transparent;
    font-size: 13px;
  }
  .markdown-preview :global(table) {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  .markdown-preview :global(th),
  .markdown-preview :global(td) {
    padding: 7px 9px;
    border: 1px solid var(--border);
    text-align: left;
  }
  .markdown-preview :global(th) { background: var(--secondary); font-weight: 600; }
  .markdown-preview :global(img) {
    max-width: 100%;
    border-radius: var(--radius-sm);
  }
  .markdown-preview :global(.katex-display) {
    overflow-x: auto;
    padding: 8px 0;
    margin: 0 0 14px;
  }
  .markdown-preview :global(.katex-error) {
    color: var(--destructive, #e05252);
    font-family: var(--font-mono);
    font-size: 0.9em;
  }
</style>
