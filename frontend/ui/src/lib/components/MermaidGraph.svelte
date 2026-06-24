<script lang="ts" module>
  /**
   * From-scratch, dependency-free renderer for simple Mermaid-style flowcharts
   * (Milestone 8). Supports `graph`/`flowchart` with `TD|TB|LR|RL` direction,
   * node shapes (`[]` `()` `{}` `(())` `([])`), and `-->` / `---` / `==>` edges
   * with optional `|labels|`. It does a small layered layout (longest-path layer
   * assignment) and draws SVG — good for the small diagrams assistants produce.
   * Anything it can't parse falls back to a labelled source box (see the caller).
   */
  type Shape = 'rect' | 'round' | 'stadium' | 'circle' | 'diamond'
  interface GNode {
    id: string
    label: string
    shape: Shape
    w: number
    h: number
    x: number
    y: number
  }
  interface GEdge {
    from: string
    to: string
    label: string
    arrow: boolean
  }
  export interface Graph {
    dir: 'TD' | 'LR'
    nodes: GNode[]
    edges: GEdge[]
    width: number
    height: number
    ok: boolean
  }

  const BOX_H = 34
  const GAP_MAIN = 50 // between layers
  const GAP_CROSS = 22 // within a layer
  const MARGIN = 12

  function clean(label: string): string {
    return label.replace(/^["']|["']$/g, '').trim()
  }

  function parseNode(tokRaw: string): GNode | null {
    const tok = tokRaw.trim()
    let m: RegExpExecArray | null
    const mk = (id: string, label: string, shape: Shape): GNode => {
      const text = clean(label) || id
      const w = Math.min(Math.max(text.length * 7 + 26, 54), 220)
      return { id, label: text, shape, w, h: BOX_H, x: 0, y: 0 }
    }
    if ((m = /^([\w.-]+)\(\((.*)\)\)$/.exec(tok))) return mk(m[1], m[2], 'circle')
    if ((m = /^([\w.-]+)\(\[(.*)\]\)$/.exec(tok))) return mk(m[1], m[2], 'stadium')
    if ((m = /^([\w.-]+)\[(.*)\]$/.exec(tok))) return mk(m[1], m[2], 'rect')
    if ((m = /^([\w.-]+)\((.*)\)$/.exec(tok))) return mk(m[1], m[2], 'round')
    if ((m = /^([\w.-]+)\{(.*)\}$/.exec(tok))) return mk(m[1], m[2], 'diamond')
    if ((m = /^([\w.-]+)$/.exec(tok))) return mk(m[1], m[1], 'rect')
    return null
  }

  export function parseGraph(source: string): Graph {
    const lines = source.split('\n').map((l) => l.trim()).filter(Boolean)
    let dir: 'TD' | 'LR' = 'TD'
    const nodes = new Map<string, GNode>()
    const edges: GEdge[] = []

    const add = (n: GNode | null) => {
      if (n && !nodes.has(n.id)) nodes.set(n.id, n)
      return n
    }

    for (const line of lines) {
      const dm = /^(?:graph|flowchart)\s+(TD|TB|LR|RL|BT)/i.exec(line)
      if (dm) {
        const d = dm[1].toUpperCase()
        dir = d === 'LR' || d === 'RL' ? 'LR' : 'TD'
        continue
      }
      if (/^(subgraph|end|classDef|class|style|click|linkStyle)\b/i.test(line)) continue

      // Split into [node, connector, node, connector, ...].
      const parts = line.split(/(-{2,3}>|={2,3}>|-\.->|---|===)/)
      if (parts.length < 3) {
        // Standalone node definition (or noise).
        add(parseNode(line))
        continue
      }
      let prevId: string | null = null
      let conn = '-->'
      for (let k = 0; k < parts.length; k++) {
        if (k % 2 === 1) {
          conn = parts[k]
          continue
        }
        let seg = parts[k].trim()
        let label = ''
        const lm = /^\|([^|]*)\|\s*/.exec(seg)
        if (lm) {
          label = clean(lm[1])
          seg = seg.slice(lm[0].length).trim()
        }
        if (!seg) continue
        const node = add(parseNode(seg))
        if (!node) continue
        if (prevId !== null) edges.push({ from: prevId, to: node.id, label, arrow: conn.includes('>') })
        prevId = node.id
      }
    }

    const ids = [...nodes.keys()]
    if (ids.length === 0) return { dir, nodes: [], edges: [], width: 0, height: 0, ok: false }

    // Longest-path layer assignment (cycle-safe: bounded relaxation).
    const layer: Record<string, number> = {}
    ids.forEach((id) => (layer[id] = 0))
    for (let iter = 0; iter < ids.length; iter++) {
      let changed = false
      for (const e of edges) {
        if (nodes.has(e.from) && nodes.has(e.to) && layer[e.to] < layer[e.from] + 1) {
          layer[e.to] = layer[e.from] + 1
          changed = true
        }
      }
      if (!changed) break
    }
    const maxLayer = Math.max(...ids.map((id) => layer[id]))
    const byLayer: string[][] = Array.from({ length: maxLayer + 1 }, () => [])
    ids.forEach((id) => byLayer[layer[id]].push(id))

    // Position. `main` axis runs across layers; `cross` axis within a layer.
    const vertical = dir === 'TD'
    let crossMax = 0
    const layerMainSize: number[] = []
    byLayer.forEach((ln) => {
      let cross = 0
      let mainSize = BOX_H
      ln.forEach((id, idx) => {
        const n = nodes.get(id)!
        const span = vertical ? n.w : n.h
        cross += span + (idx > 0 ? GAP_CROSS : 0)
        mainSize = Math.max(mainSize, vertical ? n.h : n.w)
      })
      crossMax = Math.max(crossMax, cross)
      layerMainSize.push(mainSize)
    })

    let mainPos = MARGIN
    byLayer.forEach((ln, l) => {
      let cross = 0
      ln.forEach((id, idx) => {
        const n = nodes.get(id)!
        const span = vertical ? n.w : n.h
        if (idx > 0) cross += GAP_CROSS
        const crossStart = MARGIN + (crossMax - layerCross(ln)) / 2 + cross
        if (vertical) {
          n.x = crossStart
          n.y = mainPos
        } else {
          n.x = mainPos
          n.y = crossStart
        }
        cross += span
      })
      mainPos += layerMainSize[l] + GAP_MAIN
    })

    function layerCross(ln: string[]): number {
      let c = 0
      ln.forEach((id, idx) => {
        const n = nodes.get(id)!
        c += (vertical ? n.w : n.h) + (idx > 0 ? GAP_CROSS : 0)
      })
      return c
    }

    const width = vertical ? crossMax + MARGIN * 2 : mainPos + MARGIN
    const height = vertical ? mainPos + MARGIN : crossMax + MARGIN * 2
    return { dir, nodes: [...nodes.values()], edges, width, height, ok: edges.length > 0 || ids.length > 1 }
  }
</script>

<script lang="ts">
  let { source }: { source: string } = $props()
  const g = $derived(parseGraph(source))
  const nodeById = $derived(new Map(g.nodes.map((n) => [n.id, n])))

  /** Edge endpoints: from the source's exit edge to the target's entry edge. */
  function edgePath(e: GEdge): { x1: number; y1: number; x2: number; y2: number } | null {
    const a = nodeById.get(e.from)
    const b = nodeById.get(e.to)
    if (!a || !b) return null
    if (g.dir === 'TD') {
      return { x1: a.x + a.w / 2, y1: a.y + a.h, x2: b.x + b.w / 2, y2: b.y }
    }
    return { x1: a.x + a.w, y1: a.y + a.h / 2, x2: b.x, y2: b.y + b.h / 2 }
  }
</script>

{#if g.ok}
  <div class="graph-wrap">
    <svg viewBox={`0 0 ${g.width} ${g.height}`} width={g.width} height={g.height} role="img" aria-label="Diagram">
      <defs>
        <marker id="gw-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M0,0 L10,5 L0,10 z" fill="var(--ai-text-muted, #888)" />
        </marker>
      </defs>
      {#each g.edges as e}
        {@const p = edgePath(e)}
        {#if p}
          <line
            x1={p.x1}
            y1={p.y1}
            x2={p.x2}
            y2={p.y2}
            class="gw-edge"
            marker-end={e.arrow ? 'url(#gw-arrow)' : undefined}
          />
          {#if e.label}
            <text x={(p.x1 + p.x2) / 2} y={(p.y1 + p.y2) / 2 - 3} class="gw-edge-label" text-anchor="middle">{e.label}</text>
          {/if}
        {/if}
      {/each}
      {#each g.nodes as n}
        {#if n.shape === 'diamond'}
          <polygon
            points={`${n.x + n.w / 2},${n.y} ${n.x + n.w},${n.y + n.h / 2} ${n.x + n.w / 2},${n.y + n.h} ${n.x},${n.y + n.h / 2}`}
            class="gw-node"
          />
        {:else}
          <rect
            x={n.x}
            y={n.y}
            width={n.w}
            height={n.h}
            rx={n.shape === 'rect' ? 6 : n.h / 2}
            class="gw-node"
          />
        {/if}
        <text x={n.x + n.w / 2} y={n.y + n.h / 2} class="gw-node-text" text-anchor="middle" dominant-baseline="central">{n.label}</text>
      {/each}
    </svg>
  </div>
{:else}
  <pre class="graph-fallback"><code>{source}</code></pre>
{/if}

<style>
  .graph-wrap {
    margin: 6px 0;
    padding: 8px;
    overflow-x: auto;
    background-color: var(--ai-bg-surface, var(--secondary));
    border: 1px solid var(--ai-border-subtle, var(--border));
    border-radius: 10px;
  }
  .graph-wrap svg {
    max-width: 100%;
    height: auto;
    display: block;
  }
  .gw-node {
    fill: var(--ai-bg-base, var(--background));
    stroke: var(--ai-primary, var(--primary));
    stroke-width: 1.4;
  }
  .gw-node-text {
    fill: var(--ai-text-primary, var(--foreground));
    font-family: var(--font-sans);
    font-size: 12px;
  }
  .gw-edge {
    stroke: var(--ai-text-muted, #888);
    stroke-width: 1.4;
    fill: none;
  }
  .gw-edge-label {
    fill: var(--ai-text-muted, #888);
    font-family: var(--font-sans);
    font-size: 10.5px;
  }
  .graph-fallback {
    margin: 4px 0;
    padding: 10px;
    overflow-x: auto;
    background-color: var(--ai-bg-base, var(--background));
    border: 1px solid var(--ai-border-subtle, var(--border));
    border-radius: 8px;
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre;
  }
</style>
