import { marked } from 'marked'
import katex from 'katex'

// ---------------------------------------------------------------------------
// KaTeX placeholder round-trip
// We extract math spans BEFORE marked runs so it never mangles the LaTeX.
// ---------------------------------------------------------------------------
type MathEntry = { display: boolean; src: string }

function extractMath(source: string, store: MathEntry[]): string {
  // Display math: $$...$$
  source = source.replace(/\$\$([\s\S]+?)\$\$/g, (_match, tex) => {
    const id = store.length
    store.push({ display: true, src: tex })
    return `MATHPLACEHOLDER${id}END`
  })
  // Inline math: $...$  (not inside backticks — simple heuristic)
  source = source.replace(/\$([^$\n]+?)\$/g, (_match, tex) => {
    const id = store.length
    store.push({ display: false, src: tex })
    return `MATHPLACEHOLDER${id}END`
  })
  return source
}

function reinsertMath(html: string, store: MathEntry[]): string {
  return html.replace(/MATHPLACEHOLDER(\d+)END/g, (_match, idStr) => {
    const entry = store[Number(idStr)]
    if (!entry) return ''
    try {
      return katex.renderToString(entry.src, {
        displayMode: entry.display,
        throwOnError: false,
        output: 'html',
      })
    } catch {
      return `<span class="katex-error">${entry.src}</span>`
    }
  })
}

// ---------------------------------------------------------------------------
// marked configuration — enable GFM (tables, strikethrough, task lists)
// ---------------------------------------------------------------------------
marked.setOptions({ gfm: true, breaks: false })

// Override the code block renderer to tag mermaid fences for client-side
// rendering and keep everything else as a <pre><code> block.
const renderer = new marked.Renderer()
renderer.code = ({ text, lang }) => {
  if (lang === 'mermaid') {
    return `<pre class="mermaid">${text}</pre>`
  }
  const escapedLang = lang ? ` class="language-${lang}"` : ''
  const escaped = text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
  return `<pre><code${escapedLang}>${escaped}</code></pre>`
}

marked.use({ renderer })

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------
export function renderMarkdown(source: string): string {
  const mathStore: MathEntry[] = []
  const withPlaceholders = extractMath(source, mathStore)
  const rawHtml = marked.parse(withPlaceholders) as string
  return reinsertMath(rawHtml, mathStore)
}
