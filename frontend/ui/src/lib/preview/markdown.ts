import { marked } from 'marked'
import { renderMermaid } from './mermaid-lite'

// ---------------------------------------------------------------------------
// KaTeX — loaded lazily on first math token, never touches the main bundle.
// ---------------------------------------------------------------------------
type KatexModule = typeof import('katex')
let katexMod: KatexModule | null = null
async function getKatex(): Promise<KatexModule> {
  if (!katexMod) katexMod = await import('katex')
  return katexMod
}

// ---------------------------------------------------------------------------
// KaTeX placeholder round-trip
// Extract math spans BEFORE marked runs so it never mangles the LaTeX.
// ---------------------------------------------------------------------------
type MathEntry = { display: boolean; src: string }

function extractMath(source: string, store: MathEntry[]): string {
  source = source.replace(/\$\$([\s\S]+?)\$\$/g, (_match, tex) => {
    const id = store.length
    store.push({ display: true, src: tex })
    return `MATHPLACEHOLDER${id}END`
  })
  source = source.replace(/\$([^$\n]+?)\$/g, (_match, tex) => {
    const id = store.length
    store.push({ display: false, src: tex })
    return `MATHPLACEHOLDER${id}END`
  })
  return source
}

async function reinsertMath(html: string, store: MathEntry[]): Promise<string> {
  if (!store.length) return html
  const katex = await getKatex()
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
// marked configuration — GFM (tables, strikethrough, task lists)
// ---------------------------------------------------------------------------
marked.setOptions({ gfm: true, breaks: false })

const renderer = new marked.Renderer()
renderer.code = ({ text, lang }) => {
  if (lang === 'mermaid') {
    return `<div class="mermaid-diagram">${renderMermaid(text)}</div>`
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
export async function renderMarkdown(source: string): Promise<string> {
  const mathStore: MathEntry[] = []
  const withPlaceholders = extractMath(source, mathStore)
  const rawHtml = marked.parse(withPlaceholders) as string
  return reinsertMath(rawHtml, mathStore)
}
