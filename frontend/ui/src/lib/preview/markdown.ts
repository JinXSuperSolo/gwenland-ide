import { renderMermaid } from './mermaid-lite'
import initSyntaxRenderer, {
  render_markdown,
} from '../wasm/gwenland-syntax-renderer/gwenland_syntax_renderer'
import syntaxRendererWasmUrl from '../wasm/gwenland-syntax-renderer/gwenland_syntax_renderer_bg.wasm?url'

type MermaidEntry = { id: number; html: string }

let rendererInit: Promise<void> | null = null

function ensureRenderer(): Promise<void> {
  if (!rendererInit) {
    rendererInit = initSyntaxRenderer(syntaxRendererWasmUrl).then(() => undefined)
  }
  return rendererInit
}

function extractMermaid(source: string, store: MermaidEntry[]): string {
  return source.replace(/```mermaid\s*\n([\s\S]*?)```/g, (_match, diagram) => {
    const id = store.length
    store.push({ id, html: renderMermaid(diagram.trimEnd()) })
    return `\n\nGWENMERMAIDPLACEHOLDER${id}END\n\n`
  })
}

function reinsertMermaid(html: string, store: MermaidEntry[]): string {
  if (!store.length) return html

  return html.replace(/(?:<p>)?GWENMERMAIDPLACEHOLDER(\d+)END(?:<\/p>)?/g, (_match, idStr) => {
    const entry = store[Number(idStr)]
    return entry ? `<div class="mermaid-diagram">${entry.html}</div>` : ''
  })
}

export async function renderMarkdown(source: string): Promise<string> {
  const mermaidStore: MermaidEntry[] = []
  const markdown = extractMermaid(source, mermaidStore)
  await ensureRenderer()
  return reinsertMermaid(render_markdown(markdown), mermaidStore)
}
