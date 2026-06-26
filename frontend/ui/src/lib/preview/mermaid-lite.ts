/**
 * mermaid-lite.ts — zero-dependency mermaid renderer.
 * Supports: flowchart (graph TD/LR/TB/BT/RL), sequence diagram, pie chart.
 * Returns an SVG string or a styled error block. Pure TS, no DOM required.
 */

// ─── Palette (GwenLand dark theme) ────────────────────────────────────────
const C = {
  bg:        '#1e1d1d',
  nodeFill:  '#2a2826',
  nodeBorder:'#c28a64',
  nodeText:  '#f0ece8',
  edge:      '#8a7a6e',
  edgeLabel: '#a39e9a',
  seqLife:   '#2a2826',
  seqActor:  '#c28a64',
  seqActorTx:'#1f1e1e',
  seqArrow:  '#c28a64',
  seqNote:   '#3a3330',
  seqNoteTx: '#d4ccc6',
  seqMsg:    '#d4ccc6',
  pie:       ['#c28a64','#7a9ec2','#7ac27a','#c2c27a','#c27a7a','#9a7ac2','#7ac2b8','#c29a7a'],
  pieBorder: '#1f1e1e',
  pieLabel:  '#e6e0da',
  error:     '#3a2020',
  errorBorder:'#c25a5a',
  errorText: '#e06c75',
}

// ─── SVG helpers ───────────────────────────────────────────────────────────
function esc(s: string): string {
  return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;')
}
function tx(x: number, y: number, text: string, opts: {
  fill?: string; size?: number; anchor?: string; weight?: string; dy?: number
} = {}): string {
  const fill   = opts.fill   ?? C.nodeText
  const size   = opts.size   ?? 12
  const anchor = opts.anchor ?? 'middle'
  const weight = opts.weight ?? 'normal'
  const dy     = opts.dy     ?? 0
  return `<text x="${x}" y="${y}" dy="${dy}" fill="${fill}" font-size="${size}" font-family="Inter,system-ui,sans-serif" text-anchor="${anchor}" font-weight="${weight}" dominant-baseline="central">${esc(text)}</text>`
}
function errSvg(msg: string): string {
  const w = 420, h = 60
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
<rect width="${w}" height="${h}" rx="6" fill="${C.error}" stroke="${C.errorBorder}" stroke-width="1.5"/>
${tx(w/2, h/2, '⚠ ' + msg, { fill: C.errorText, size: 12, anchor: 'middle' })}
</svg>`
}

// ─── Flowchart ─────────────────────────────────────────────────────────────
type FNode = { id: string; label: string; shape: 'rect'|'round'|'diamond'|'stadium' }
type FEdge = { from: string; to: string; label: string; arrow: '-->'|'---'|'-.->'|'==>' }
type Dir   = 'TD'|'LR'|'TB'|'BT'|'RL'

function parseFlowchart(src: string): { nodes: FNode[]; edges: FEdge[]; dir: Dir } {
  const lines = src.split('\n').map(l => l.trim()).filter(Boolean)
  const header = lines[0] ?? ''
  const dirMatch = header.match(/\b(TD|LR|TB|BT|RL)\b/i)
  const dir: Dir = (dirMatch?.[1]?.toUpperCase() as Dir) ?? 'TD'

  const nodes = new Map<string, FNode>()
  const edges: FEdge[] = []

  function ensureNode(id: string, label?: string, shape?: FNode['shape']): FNode {
    if (!nodes.has(id)) {
      nodes.set(id, { id, label: label ?? id, shape: shape ?? 'rect' })
    } else if (label) {
      const n = nodes.get(id)!
      n.label = label
      if (shape) n.shape = shape
    }
    return nodes.get(id)!
  }

  // Parse node definitions and edges
  // Supports: A[label], A(label), A{label}, A([label]), A-->B, A-->|label|B, A--text-->B
  const edgeRe = /^(.+?)\s*(--?>|---|-\.->|==>)\s*(?:\|([^|]*)\|)?\s*(.+)$/
  const nodeRe = /^([A-Za-z0-9_]+)\s*(\[([^\]]*)\]|\(([^)]*)\)|\{([^}]*)\}|\(\[([^\]]*)\]\))?/

  for (const line of lines.slice(1)) {
    if (!line || line.startsWith('%%')) continue
    const em = line.match(edgeRe)
    if (em) {
      const [, fromRaw, arrowRaw, edgeLabel, toRaw] = em
      // Parse from node
      const fm = fromRaw.trim().match(nodeRe)
      const tm = toRaw.trim().match(nodeRe)
      if (!fm || !tm) continue
      const fromId = fm[1]
      const fromLabel = fm[3] ?? fm[4] ?? fm[5] ?? fm[6] ?? fromId
      const fromShape: FNode['shape'] = fm[3] ? 'rect' : fm[4] ? 'round' : fm[5] ? 'diamond' : fm[6] ? 'stadium' : 'rect'
      const toId = tm[1]
      const toLabel = tm[3] ?? tm[4] ?? tm[5] ?? tm[6] ?? toId
      const toShape: FNode['shape'] = tm[3] ? 'rect' : tm[4] ? 'round' : tm[5] ? 'diamond' : tm[6] ? 'stadium' : 'rect'
      ensureNode(fromId, fromLabel, fromShape)
      ensureNode(toId, toLabel, toShape)
      const arrow = arrowRaw.includes('=') ? '==>' : arrowRaw.includes('.') ? '-.->' : arrowRaw.includes('>') ? '-->' : '---'
      edges.push({ from: fromId, to: toId, label: edgeLabel ?? '', arrow: arrow as FEdge['arrow'] })
    } else {
      const nm = line.match(nodeRe)
      if (nm) {
        const id = nm[1]
        const label = nm[3] ?? nm[4] ?? nm[5] ?? nm[6] ?? id
        const shape: FNode['shape'] = nm[3] ? 'rect' : nm[4] ? 'round' : nm[5] ? 'diamond' : nm[6] ? 'stadium' : 'rect'
        ensureNode(id, label, shape)
      }
    }
  }

  return { nodes: Array.from(nodes.values()), edges, dir }
}

function layoutFlowchart(nodes: FNode[], edges: FEdge[], dir: Dir): Map<string, {x:number;y:number}> {
  // Simple layered layout: topological sort → assign levels → spread per level
  const pos = new Map<string, {x:number;y:number}>()
  if (!nodes.length) return pos

  // Build adjacency
  const children = new Map<string, string[]>()
  const parents  = new Map<string, string[]>()
  nodes.forEach(n => { children.set(n.id, []); parents.set(n.id, []) })
  edges.forEach(e => {
    children.get(e.from)?.push(e.to)
    parents.get(e.to)?.push(e.from)
  })

  // Topological sort (Kahn's)
  const level = new Map<string, number>()
  const queue = nodes.filter(n => (parents.get(n.id)?.length ?? 0) === 0).map(n => n.id)
  queue.forEach(id => level.set(id, 0))
  const visited = new Set<string>(queue)
  let qi = 0
  while (qi < queue.length) {
    const id = queue[qi++]
    const l = level.get(id) ?? 0
    for (const child of (children.get(id) ?? [])) {
      const newL = l + 1
      if (!level.has(child) || (level.get(child)! < newL)) level.set(child, newL)
      if (!visited.has(child)) { visited.add(child); queue.push(child) }
    }
  }
  // Any node not visited (cycle) gets level 0
  nodes.forEach(n => { if (!level.has(n.id)) level.set(n.id, 0) })

  // Group by level
  const byLevel = new Map<number, string[]>()
  level.forEach((l, id) => {
    if (!byLevel.has(l)) byLevel.set(l, [])
    byLevel.get(l)!.push(id)
  })

  const nodeW = 140, nodeH = 40, hGap = 60, vGap = 70
  const horizontal = dir === 'LR' || dir === 'RL'

  byLevel.forEach((ids, l) => {
    ids.forEach((id, i) => {
      const across = (i - (ids.length - 1) / 2) * (nodeW + hGap)
      const along  = l * (nodeH + vGap)
      if (horizontal) {
        pos.set(id, { x: along + nodeW / 2, y: across })
      } else {
        pos.set(id, { x: across, y: along + nodeH / 2 })
      }
    })
  })

  if (dir === 'BT' || dir === 'RL') {
    const maxAlong = Math.max(...[...pos.values()].map(p => horizontal ? p.x : p.y))
    pos.forEach((p, id) => {
      if (horizontal) pos.set(id, { ...p, x: maxAlong - p.x + nodeW / 2 })
      else            pos.set(id, { ...p, y: maxAlong - p.y + nodeH / 2 })
    })
  }

  return pos
}

function renderNode(n: FNode, cx: number, cy: number): string {
  const w = 130, h = 34, rx = 6
  const x = cx - w/2, y = cy - h/2
  const strokeDash = ''
  let shape = ''
  if (n.shape === 'diamond') {
    const hw = w/2, hh = h/2 + 8
    shape = `<polygon points="${cx},${cy-hh} ${cx+hw},${cy} ${cx},${cy+hh} ${cx-hw},${cy}" fill="${C.nodeFill}" stroke="${C.nodeBorder}" stroke-width="1.5"/>`
  } else if (n.shape === 'round') {
    shape = `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${h/2}" fill="${C.nodeFill}" stroke="${C.nodeBorder}" stroke-width="1.5" ${strokeDash}/>`
  } else if (n.shape === 'stadium') {
    shape = `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${h/2}" fill="${C.nodeFill}" stroke="${C.nodeBorder}" stroke-width="1.5" stroke-dasharray="4 2"/>`
  } else {
    shape = `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${rx}" fill="${C.nodeFill}" stroke="${C.nodeBorder}" stroke-width="1.5"/>`
  }
  return shape + tx(cx, cy, n.label, { size: 12 })
}

function arrowHead(x: number, y: number, angle: number, dashed: boolean): string {
  const size = 8
  const cos = Math.cos(angle), sin = Math.sin(angle)
  const tip = { x, y }
  const left  = { x: x - size*cos + size/2*sin, y: y - size*sin - size/2*cos }
  const right = { x: x - size*cos - size/2*sin, y: y - size*sin + size/2*cos }
  const fill = dashed ? 'none' : C.nodeBorder
  return `<polygon points="${tip.x},${tip.y} ${left.x},${left.y} ${right.x},${right.y}" fill="${fill}" stroke="${C.nodeBorder}" stroke-width="1"/>`
}

function renderFlowchart(src: string): string {
  const { nodes, edges, dir } = parseFlowchart(src)
  if (!nodes.length) return errSvg('No nodes found in flowchart')

  const pos = layoutFlowchart(nodes, edges, dir)

  // Compute bounding box
  const xs = [...pos.values()].map(p => p.x)
  const ys = [...pos.values()].map(p => p.y)
  const pad = 50
  const minX = Math.min(...xs) - 80
  const minY = Math.min(...ys) - 40
  const maxX = Math.max(...xs) + 80
  const maxY = Math.max(...ys) + 40
  const W = maxX - minX + pad * 2
  const H = maxY - minY + pad * 2
  const ox = pad - minX, oy = pad - minY

  const edgeSvg: string[] = []
  const arrowSvg: string[] = []
  const labelSvg: string[] = []

  edges.forEach(e => {
    const fp = pos.get(e.from), tp = pos.get(e.to)
    if (!fp || !tp) return
    const fx = fp.x + ox, fy = fp.y + oy
    const tx2 = tp.x + ox, ty = tp.y + oy
    // Offset end point to node border
    const dx = tx2 - fx, dy = ty - fy
    const len = Math.sqrt(dx*dx + dy*dy) || 1
    const ex = tx2 - dx/len * 18, ey = ty - dy/len * 18
    const dashed = e.arrow === '-.->' || e.arrow === '---'
    const strokeDash = dashed ? 'stroke-dasharray="6 3"' : ''
    const noArrow = e.arrow === '---'
    edgeSvg.push(`<line x1="${fx}" y1="${fy}" x2="${ex}" y2="${ey}" stroke="${C.edge}" stroke-width="1.5" ${strokeDash}/>`)
    if (!noArrow) {
      const angle = Math.atan2(ey - fy, ex - fx)
      arrowSvg.push(arrowHead(ex, ey, angle, dashed))
    }
    if (e.label) {
      const mx = (fx + tx2) / 2, my = (fy + ty) / 2
      labelSvg.push(`<rect x="${mx-24}" y="${my-9}" width="48" height="16" rx="3" fill="${C.bg}" opacity="0.85"/>`)
      labelSvg.push(tx(mx, my, e.label, { fill: C.edgeLabel, size: 10 }))
    }
  })

  const nodeSvg = nodes.map(n => {
    const p = pos.get(n.id)
    if (!p) return ''
    return renderNode(n, p.x + ox, p.y + oy)
  })

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
<rect width="${W}" height="${H}" fill="${C.bg}" rx="8"/>
${edgeSvg.join('\n')}
${arrowSvg.join('\n')}
${labelSvg.join('\n')}
${nodeSvg.join('\n')}
</svg>`
}

// ─── Sequence diagram ──────────────────────────────────────────────────────
type SeqMsg = { from: string; to: string; text: string; type: 'solid'|'dashed'|'self' }
type SeqNote = { over: string; text: string }
type SeqItem = ({ kind: 'msg' } & SeqMsg) | ({ kind: 'note' } & SeqNote) | { kind: 'divider'; text: string }

function parseSequence(src: string): { actors: string[]; items: SeqItem[] } {
  const lines = src.split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('%%') && !l.match(/^sequenceDiagram/i))
  const actorOrder: string[] = []
  const actorSet = new Set<string>()
  const items: SeqItem[] = []

  function addActor(name: string) {
    const clean = name.replace(/^["']|["']$/g,'').trim()
    if (!actorSet.has(clean)) { actorSet.add(clean); actorOrder.push(clean) }
    return clean
  }

  for (const line of lines) {
    // participant / actor declarations
    const partMatch = line.match(/^(?:participant|actor)\s+(.+?)(?:\s+as\s+(.+))?$/)
    if (partMatch) { addActor(partMatch[2] ?? partMatch[1]); continue }

    // note over / note left of / note right of
    const noteMatch = line.match(/^note\s+(?:over|left of|right of)\s+([^:]+):\s*(.+)$/i)
    if (noteMatch) { const over = addActor(noteMatch[1].trim()); items.push({ kind: 'note', over, text: noteMatch[2] }); continue }

    // dividers
    if (line.match(/^(?:rect|loop|alt|else|opt|par|and|critical|break|end)\b/i)) {
      const divMatch = line.match(/^(\w+)\s+(.*)/)
      items.push({ kind: 'divider', text: divMatch ? `${divMatch[1].toUpperCase()} ${divMatch[2]}` : line.toUpperCase() })
      continue
    }

    // messages: A->>B: text / A->B: text / A-->B: text
    const msgMatch = line.match(/^([^-]+?)\s*(->>?|-->>?|->|-->)\s*([^:]+?):\s*(.*)$/)
    if (msgMatch) {
      const from = addActor(msgMatch[1].trim())
      const to   = addActor(msgMatch[3].trim())
      const text = msgMatch[4].trim()
      const arrowStr = msgMatch[2]
      const type: SeqMsg['type'] = from === to ? 'self' : arrowStr.includes('--') ? 'dashed' : 'solid'
      items.push({ kind: 'msg', from, to, text, type })
    }
  }

  return { actors: actorOrder, items }
}

function renderSequence(src: string): string {
  const { actors, items } = parseSequence(src)
  if (!actors.length) return errSvg('No actors found in sequence diagram')

  const actorW = 110, actorH = 30, hGap = 60
  const totalActorW = actors.length * actorW + (actors.length - 1) * hGap
  const pad = 30
  const W = totalActorW + pad * 2

  // Actor center x positions
  const ax = new Map<string, number>()
  actors.forEach((a, i) => ax.set(a, pad + i * (actorW + hGap) + actorW / 2))

  // First pass: measure height
  let curY = actorH + 20
  const rowH = 40
  const rows: Array<{ y: number; item: SeqItem }> = []
  items.forEach(item => {
    rows.push({ y: curY, item })
    curY += rowH
  })
  const H = curY + actorH + 20

  const out: string[] = []
  out.push(`<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">`)
  out.push(`<rect width="${W}" height="${H}" fill="${C.bg}" rx="8"/>`)

  // Lifelines
  actors.forEach(a => {
    const x = ax.get(a)!
    out.push(`<line x1="${x}" y1="${actorH}" x2="${x}" y2="${H - actorH}" stroke="${C.edge}" stroke-width="1" stroke-dasharray="4 4" opacity="0.5"/>`)
  })

  // Actor boxes top
  actors.forEach(a => {
    const x = ax.get(a)!
    const bx = x - actorW/2, by = 0
    out.push(`<rect x="${bx}" y="${by}" width="${actorW}" height="${actorH}" rx="4" fill="${C.seqActor}" stroke="none"/>`)
    out.push(tx(x, by + actorH/2, a, { fill: C.seqActorTx, size: 11, weight: '600' }))
  })

  // Messages and notes
  rows.forEach(({ y, item }) => {
    if (item.kind === 'msg') {
      const fx = ax.get(item.from)!, tx2 = ax.get(item.to)!
      if (item.type === 'self') {
        const lw = 40
        out.push(`<path d="M${fx},${y} C${fx+lw},${y} ${fx+lw},${y+24} ${fx},${y+24}" fill="none" stroke="${C.seqArrow}" stroke-width="1.5" ${item.type==='self'&&item.type?'':''}/>`)
        out.push(arrowHead(fx, y+24, Math.PI/2+0.3, false))
        out.push(tx(fx + lw + 6, y + 12, item.text, { fill: C.seqMsg, size: 11, anchor: 'start' }))
      } else {
        const dashed = item.type === 'dashed'
        const dir2 = tx2 > fx ? 1 : -1
        const endX = tx2 - dir2 * 8
        out.push(`<line x1="${fx}" y1="${y}" x2="${endX}" y2="${y}" stroke="${C.seqArrow}" stroke-width="1.5" ${dashed?'stroke-dasharray="5 3"':''}/>`)
        out.push(arrowHead(endX, y, dir2 > 0 ? 0 : Math.PI, false))
        const midX = (fx + tx2) / 2
        out.push(`<rect x="${midX-36}" y="${y-14}" width="72" height="13" rx="2" fill="${C.bg}" opacity="0.8"/>`)
        out.push(tx(midX, y - 7, item.text, { fill: C.seqMsg, size: 10 }))
      }
    } else if (item.kind === 'note') {
      const x = ax.get(item.over) ?? W/2
      const nw = 100, nh = 22
      out.push(`<rect x="${x-nw/2}" y="${y-nh/2}" width="${nw}" height="${nh}" rx="4" fill="${C.seqNote}" stroke="${C.edge}" stroke-width="1"/>`)
      out.push(tx(x, y, item.text, { fill: C.seqNoteTx, size: 10 }))
    } else if (item.kind === 'divider') {
      out.push(`<line x1="${pad}" y1="${y}" x2="${W-pad}" y2="${y}" stroke="${C.edge}" stroke-width="1" stroke-dasharray="3 3" opacity="0.4"/>`)
      out.push(tx(W/2, y - 8, item.text, { fill: C.edgeLabel, size: 10 }))
    }
  })

  // Actor boxes bottom
  actors.forEach(a => {
    const x = ax.get(a)!
    const bx = x - actorW/2, by = H - actorH
    out.push(`<rect x="${bx}" y="${by}" width="${actorW}" height="${actorH}" rx="4" fill="${C.seqActor}" stroke="none"/>`)
    out.push(tx(x, by + actorH/2, a, { fill: C.seqActorTx, size: 11, weight: '600' }))
  })

  out.push('</svg>')
  return out.join('\n')
}

// ─── Pie chart ─────────────────────────────────────────────────────────────
function renderPie(src: string): string {
  const lines = src.split('\n').map(l => l.trim()).filter(Boolean)
  // Title line (optional)
  const titleLine = lines.find(l => l.startsWith('title '))
  const title = titleLine?.slice(6).trim() ?? ''
  const entries: { label: string; value: number }[] = []

  for (const line of lines) {
    const m = line.match(/^"([^"]+)"\s*:\s*([\d.]+)/)
    if (m) entries.push({ label: m[1], value: parseFloat(m[2]) })
  }

  if (!entries.length) return errSvg('No data entries in pie chart')

  const total = entries.reduce((s, e) => s + e.value, 0)
  const cx = 160, cy = title ? 180 : 160, r = 110
  const legendX = cx * 2 + 20
  const W = legendX + 180
  const H = Math.max(cy + r + 20, entries.length * 22 + 40)
  const out: string[] = []
  out.push(`<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">`)
  out.push(`<rect width="${W}" height="${H}" fill="${C.bg}" rx="8"/>`)

  if (title) out.push(tx(cx, 24, title, { fill: C.nodeText, size: 14, weight: '600' }))

  let startAngle = -Math.PI / 2
  entries.forEach((e, i) => {
    const slice = (e.value / total) * Math.PI * 2
    const endAngle = startAngle + slice
    const x1 = cx + r * Math.cos(startAngle)
    const y1 = cy + r * Math.sin(startAngle)
    const x2 = cx + r * Math.cos(endAngle)
    const y2 = cy + r * Math.sin(endAngle)
    const large = slice > Math.PI ? 1 : 0
    const color = C.pie[i % C.pie.length]
    out.push(`<path d="M${cx},${cy} L${x1},${y1} A${r},${r} 0 ${large},1 ${x2},${y2} Z" fill="${color}" stroke="${C.pieBorder}" stroke-width="2"/>`)

    // Percentage label inside slice
    const midA = startAngle + slice / 2
    const lr = r * 0.65
    const lx = cx + lr * Math.cos(midA)
    const ly = cy + lr * Math.sin(midA)
    const pct = ((e.value / total) * 100).toFixed(0) + '%'
    if (slice > 0.2) out.push(tx(lx, ly, pct, { fill: '#fff', size: 11, weight: '600' }))

    // Legend
    const legendY = 30 + i * 22
    out.push(`<rect x="${legendX}" y="${legendY - 7}" width="14" height="14" rx="3" fill="${color}"/>`)
    out.push(tx(legendX + 20, legendY, e.label, { fill: C.pieLabel, size: 11, anchor: 'start' }))
    out.push(tx(legendX + 20, legendY + 13, String(e.value), { fill: C.edgeLabel, size: 10, anchor: 'start' }))

    startAngle = endAngle
  })

  out.push('</svg>')
  return out.join('\n')
}

// ─── Public entry point ────────────────────────────────────────────────────
export function renderMermaid(source: string): string {
  const src = source.trim()
  const firstLine = src.split('\n')[0].toLowerCase().trim()
  try {
    if (firstLine.startsWith('graph') || firstLine.startsWith('flowchart')) {
      return renderFlowchart(src)
    }
    if (firstLine.startsWith('sequencediagram')) {
      return renderSequence(src)
    }
    if (firstLine.startsWith('pie')) {
      return renderPie(src)
    }
    return errSvg(`Unsupported diagram type: "${src.split('\n')[0].trim()}"`)
  } catch (e) {
    return errSvg(String(e).slice(0, 80))
  }
}
