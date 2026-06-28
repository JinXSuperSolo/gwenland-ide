<script lang="ts">
  import { onMount } from 'svelte'
  import { gitGraph, loadGitGraph, refreshGitGraph } from '../../stores/gitGraph'
  import type { CommitEdge, CommitNode } from '../../types/git'
  import GitGraphDock from './GitGraphDock.svelte'
  import GitGraphPopup from './GitGraphPopup.svelte'
  import GitGraphTooltip from './GitGraphTooltip.svelte'

  const COMMIT_SPACING = 60
  const LANE_HEIGHT = 32
  const NODE_RADIUS = 6
  const MIN_SCALE = 0.4
  const MAX_SCALE = 3
  const BRANCH_COLORS = [
    '#e18445',
    '#7c9eff',
    '#6ee7b7',
    '#f472b6',
    '#a78bfa',
    '#fbbf24',
    '#34d399',
    '#60a5fa',
  ]

  let {
    workspacePath,
  }: {
    workspacePath: string
  } = $props()

  let wrapper: HTMLDivElement
  let canvas: HTMLCanvasElement
  let ctx: CanvasRenderingContext2D | null = null
  let resizeObserver: ResizeObserver | null = null
  let rafPending = false
  let fittedKey: string | null = null
  let dockHideTimer: number | null = null
  let panAnimation: number | null = null

  let panX = $state(40)
  let panY = $state(60)
  let scale = $state(1)
  let hoveredNode = $state<CommitNode | null>(null)
  let selectedNode = $state<CommitNode | null>(null)
  let tooltipPos = $state({ x: 0, y: 0 })
  let dragging = $state(false)
  let dragMoved = false
  let lastMouse = { x: 0, y: 0 }
  let slowLoading = $state(false)
  let dockVisible = $state(true)

  const graphData = $derived($gitGraph.payload)
  const nodes = $derived(graphData?.nodes ?? [])
  const edges = $derived(graphData?.edges ?? [])
  const branches = $derived(graphData?.branches ?? [])
  const loading = $derived($gitGraph.loading)
  const error = $derived($gitGraph.error)
  const laneCount = $derived(
    nodes.reduce((max, node) => Math.max(max, node.lane + 1), 0)
  )

  onMount(() => {
    ctx = canvas.getContext('2d')
    resizeObserver = new ResizeObserver(() => resizeCanvas())
    resizeObserver.observe(wrapper)
    resizeCanvas()

    return () => {
      resizeObserver?.disconnect()
      if (dockHideTimer !== null) window.clearTimeout(dockHideTimer)
      if (panAnimation !== null) cancelAnimationFrame(panAnimation)
    }
  })

  $effect(() => {
    if (!workspacePath) return
    selectedNode = null
    hoveredNode = null
    dockVisible = true
    fittedKey = null
    void loadGitGraph(workspacePath, 300)
  })

  $effect(() => {
    const key = graphData ? `${workspacePath}:${graphData.head ?? 'none'}:${graphData.nodes.length}` : null
    if (!graphData || key === fittedKey) {
      invalidate()
      return
    }
    fittedKey = key
    window.setTimeout(() => {
      fitGraphToView()
      invalidate()
    }, 0)
  })

  $effect(() => {
    if (!loading) {
      slowLoading = false
      return
    }
    slowLoading = false
    const timer = window.setTimeout(() => {
      slowLoading = true
    }, 2000)
    return () => window.clearTimeout(timer)
  })

  function getBranchColor(lane: number): string {
    return BRANCH_COLORS[lane % BRANCH_COLORS.length]
  }

  function rgba(hex: string, alpha: number): string {
    const value = hex.replace('#', '')
    const r = parseInt(value.slice(0, 2), 16)
    const g = parseInt(value.slice(2, 4), 16)
    const b = parseInt(value.slice(4, 6), 16)
    return `rgba(${r}, ${g}, ${b}, ${alpha})`
  }

  function nodeWorldX(node: CommitNode): number {
    return node.x * COMMIT_SPACING
  }

  function nodeWorldY(node: CommitNode): number {
    return Number.isFinite(node.y) ? node.y : node.lane * LANE_HEIGHT
  }

  function laneWorldY(lane: number): number {
    return lane * LANE_HEIGHT
  }

  function canvasSize() {
    const dpr = window.devicePixelRatio || 1
    return {
      width: canvas.width / dpr,
      height: canvas.height / dpr,
    }
  }

  function visibleBounds() {
    const { width, height } = canvasSize()
    return {
      minX: -panX / scale - COMMIT_SPACING,
      maxX: (width - panX) / scale + COMMIT_SPACING,
      minY: -panY / scale - LANE_HEIGHT,
      maxY: (height - panY) / scale + LANE_HEIGHT,
    }
  }

  function resizeCanvas() {
    if (!wrapper || !canvas) return
    const rect = wrapper.getBoundingClientRect()
    const dpr = window.devicePixelRatio || 1
    const width = Math.max(1, Math.floor(rect.width))
    const height = Math.max(1, Math.floor(rect.height))
    canvas.width = Math.floor(width * dpr)
    canvas.height = Math.floor(height * dpr)
    canvas.style.width = `${width}px`
    canvas.style.height = `${height}px`
    invalidate()
  }

  function fitGraphToView() {
    if (!wrapper || nodes.length === 0) return
    const rect = wrapper.getBoundingClientRect()
    const maxX = nodes.reduce((max, node) => Math.max(max, nodeWorldX(node)), 0)
    const maxY = Math.max(0, (laneCount - 1) * LANE_HEIGHT)
    scale = Math.max(MIN_SCALE, Math.min(1, (rect.width - 56) / Math.max(maxX + 1, 1)))
    panX = Math.min(42, rect.width - maxX * scale - 56)
    panY = Math.max(42, (rect.height - maxY * scale) * 0.3)
  }

  function invalidate() {
    if (rafPending) return
    rafPending = true
    requestAnimationFrame(() => {
      rafPending = false
      render()
    })
  }

  function temporarilyHideDock() {
    dockVisible = false
    if (dockHideTimer !== null) window.clearTimeout(dockHideTimer)
    dockHideTimer = window.setTimeout(() => {
      dockVisible = true
      dockHideTimer = null
    }, 500)
  }

  function cancelPanAnimation() {
    if (panAnimation === null) return
    cancelAnimationFrame(panAnimation)
    panAnimation = null
  }

  function easeOutCubic(t: number): number {
    return 1 - Math.pow(1 - t, 3)
  }

  function animatePanTo(targetPanX: number, targetPanY: number) {
    cancelPanAnimation()
    const startPanX = panX
    const startPanY = panY
    const started = performance.now()
    const duration = 240

    const step = (now: number) => {
      const t = Math.min(1, (now - started) / duration)
      const eased = easeOutCubic(t)
      panX = startPanX + (targetPanX - startPanX) * eased
      panY = startPanY + (targetPanY - startPanY) * eased
      invalidate()
      if (t < 1) {
        panAnimation = requestAnimationFrame(step)
      } else {
        panAnimation = null
      }
    }

    panAnimation = requestAnimationFrame(step)
  }

  function focusNode(node: CommitNode) {
    if (!canvas) return
    const { width, height } = canvasSize()
    selectedNode = node
    hoveredNode = null
    const targetPanX = width / 2 - nodeWorldX(node) * scale
    const targetPanY = height / 2 - nodeWorldY(node) * scale
    animatePanTo(targetPanX, targetPanY)
  }

  function drawLaneLines(bounds: ReturnType<typeof visibleBounds>) {
    if (!ctx) return
    for (let lane = 0; lane < laneCount; lane += 1) {
      const y = laneWorldY(lane)
      if (y < bounds.minY || y > bounds.maxY) continue
      ctx.beginPath()
      ctx.lineWidth = lane === 0 ? 1.6 : 1
      ctx.strokeStyle = rgba(getBranchColor(lane), lane === 0 ? 0.26 : 0.16)
      ctx.moveTo(bounds.minX, y)
      ctx.lineTo(bounds.maxX, y)
      ctx.stroke()
    }
  }

  function drawEdge(edge: CommitEdge) {
    if (!ctx) return
    const fromX = edge.fromX * COMMIT_SPACING
    const toX = edge.toX * COMMIT_SPACING
    const fromY = laneWorldY(edge.fromLane)
    const toY = laneWorldY(edge.toLane)
    ctx.beginPath()
    ctx.lineWidth = edge.kind === 'linear' ? 2 : 1.6
    ctx.strokeStyle = rgba(getBranchColor(edge.kind === 'fork' ? edge.toLane : edge.fromLane), 0.82)
    ctx.moveTo(fromX, fromY)
    if (fromY === toY) {
      ctx.lineTo(toX, toY)
    } else {
      const midX = (fromX + toX) / 2
      ctx.bezierCurveTo(midX, fromY, midX, toY, toX, toY)
    }
    ctx.stroke()
  }

  function drawEdges(bounds: ReturnType<typeof visibleBounds>) {
    for (const edge of edges) {
      const fromX = edge.fromX * COMMIT_SPACING
      const toX = edge.toX * COMMIT_SPACING
      const minX = Math.min(fromX, toX)
      const maxX = Math.max(fromX, toX)
      const minY = Math.min(laneWorldY(edge.fromLane), laneWorldY(edge.toLane))
      const maxY = Math.max(laneWorldY(edge.fromLane), laneWorldY(edge.toLane))
      if (maxX < bounds.minX || minX > bounds.maxX || maxY < bounds.minY || minY > bounds.maxY) {
        continue
      }
      drawEdge(edge)
    }
  }

  function refLabel(ref: string): string {
    return ref
      .replace(/^refs\/heads\//, '')
      .replace(/^refs\/remotes\//, '')
      .replace(/^refs\/tags\//, '')
  }

  function clippedLabel(label: string, max = 30): string {
    return label.length > max ? `${label.slice(0, max - 3).trimEnd()}...` : label
  }

  function roundedRect(x: number, y: number, width: number, height: number, radius: number) {
    if (!ctx) return
    const r = Math.min(radius, width / 2, height / 2)
    ctx.beginPath()
    ctx.moveTo(x + r, y)
    ctx.lineTo(x + width - r, y)
    ctx.quadraticCurveTo(x + width, y, x + width, y + r)
    ctx.lineTo(x + width, y + height - r)
    ctx.quadraticCurveTo(x + width, y + height, x + width - r, y + height)
    ctx.lineTo(x + r, y + height)
    ctx.quadraticCurveTo(x, y + height, x, y + height - r)
    ctx.lineTo(x, y + r)
    ctx.quadraticCurveTo(x, y, x + r, y)
  }

  function drawLabel(x: number, y: number, label: string, color: string, filled = false) {
    if (!ctx) return
    const text = clippedLabel(label)
    ctx.font = '700 10.5px "Cascadia Mono", Consolas, monospace'
    const width = Math.min(190, Math.ceil(ctx.measureText(text).width) + 14)
    const height = 18
    roundedRect(x, y, width, height, 5)
    ctx.fillStyle = filled ? color : rgba('#1f1e1e', 0.92)
    ctx.fill()
    ctx.lineWidth = 1
    ctx.strokeStyle = filled ? rgba('#fafafa', 0.42) : rgba(color, 0.52)
    ctx.stroke()
    ctx.fillStyle = filled ? '#1f1e1e' : color
    ctx.fillText(text, x + 7, y + 12.6)
  }

  function drawBranchLabels(bounds: ReturnType<typeof visibleBounds>) {
    if (!ctx || !graphData) return
    const drawn = new Set<string>()
    for (const branch of branches) {
      const node =
        nodes.find((candidate) => candidate.hash === branch.hash) ??
        nodes.find((candidate) => candidate.refs.map(refLabel).includes(refLabel(branch.name)))
      if (!node) continue
      const x = nodeWorldX(node)
      const y = nodeWorldY(node)
      if (x < bounds.minX || x > bounds.maxX || y < bounds.minY - 34 || y > bounds.maxY + 12) continue
      const label = refLabel(branch.name)
      const key = `${label}:${node.hash}`
      if (drawn.has(key)) continue
      drawn.add(key)
      drawLabel(x + 10, y - 27, label, getBranchColor(branch.lane), false)
    }
  }

  function drawHeadBadge(node: CommitNode) {
    if (!ctx) return
    drawLabel(nodeWorldX(node) + 10, nodeWorldY(node) + 8, 'HEAD', getBranchColor(node.lane), true)
  }

  function drawNodes(bounds: ReturnType<typeof visibleBounds>) {
    if (!ctx) return
    for (const node of nodes) {
      const x = nodeWorldX(node)
      const y = nodeWorldY(node)
      if (
        x < bounds.minX ||
        x > bounds.maxX ||
        y < bounds.minY ||
        y > bounds.maxY
      ) {
        continue
      }

      const selected = selectedNode?.hash === node.hash
      const hovered = hoveredNode?.hash === node.hash
      ctx.beginPath()
      ctx.arc(x, y, NODE_RADIUS, 0, Math.PI * 2)
      ctx.fillStyle = getBranchColor(node.lane)
      ctx.fill()
      ctx.lineWidth = selected ? 3 : hovered || node.isHead ? 2 : 1.2
      ctx.strokeStyle = selected
        ? '#ffffff'
        : node.isHead
          ? '#fafafa'
          : rgba('#1f1e1e', 0.94)
      ctx.stroke()

      if (node.isMerge) {
        ctx.beginPath()
        ctx.arc(x, y, NODE_RADIUS + 3, 0, Math.PI * 2)
        ctx.lineWidth = 1
        ctx.strokeStyle = rgba('#fafafa', 0.35)
        ctx.stroke()
      }

      if (node.isHead) {
        drawHeadBadge(node)
      }
    }
  }

  function drawSelectedHighlight() {
    if (!ctx || !selectedNode) return
    const x = nodeWorldX(selectedNode)
    const y = nodeWorldY(selectedNode)
    ctx.beginPath()
    ctx.arc(x, y, NODE_RADIUS + 8, 0, Math.PI * 2)
    ctx.lineWidth = 1.4
    ctx.strokeStyle = rgba('#e18445', 0.85)
    ctx.stroke()
  }

  function render() {
    if (!ctx || !canvas) return
    const dpr = window.devicePixelRatio || 1
    const width = canvas.width / dpr
    const height = canvas.height / dpr
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, height)

    if (!graphData || graphData.nodes.length === 0) return

    const bounds = visibleBounds()
    ctx.save()
    ctx.translate(panX, panY)
    ctx.scale(scale, scale)
    drawEdges(bounds)
    drawLaneLines(bounds)
    drawNodes(bounds)
    drawBranchLabels(bounds)
    drawSelectedHighlight()
    ctx.restore()
  }

  function findNodeAtScreen(screenX: number, screenY: number): CommitNode | null {
    if (!graphData) return null
    const worldX = (screenX - panX) / scale
    const worldY = (screenY - panY) / scale
    const threshold = Math.max(NODE_RADIUS * 2, 12 / scale)
    const bounds = visibleBounds()

    for (const node of nodes) {
      const x = nodeWorldX(node)
      const y = nodeWorldY(node)
      if (
        x < bounds.minX ||
        x > bounds.maxX ||
        y < bounds.minY ||
        y > bounds.maxY
      ) {
        continue
      }
      const dx = x - worldX
      const dy = y - worldY
      if (Math.sqrt(dx * dx + dy * dy) <= threshold) {
        return node
      }
    }
    return null
  }

  function pointerPoint(e: MouseEvent) {
    const rect = canvas.getBoundingClientRect()
    return {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    }
  }

  function onMouseDown(e: MouseEvent) {
    cancelPanAnimation()
    dragging = true
    dragMoved = false
    lastMouse = { x: e.clientX, y: e.clientY }
  }

  function onMouseMove(e: MouseEvent) {
    const point = pointerPoint(e)
    if (dragging) {
      const dx = e.clientX - lastMouse.x
      const dy = e.clientY - lastMouse.y
      if (Math.abs(dx) + Math.abs(dy) > 2) dragMoved = true
      panX += dx
      panY += dy
      lastMouse = { x: e.clientX, y: e.clientY }
      temporarilyHideDock()
      invalidate()
      return
    }

    const node = findNodeAtScreen(point.x, point.y)
    tooltipPos = clampOverlay(point.x + 12, point.y + 12, 260, 112)
    if (hoveredNode?.hash !== node?.hash) {
      hoveredNode = node
      invalidate()
    }
  }

  function onMouseUp(e: MouseEvent) {
    const point = pointerPoint(e)
    const wasDragging = dragging
    dragging = false
    if (!wasDragging || dragMoved) return
    const node = findNodeAtScreen(point.x, point.y)
    selectedNode = node
    invalidate()
  }

  function onMouseLeave() {
    dragging = false
    if (hoveredNode) {
      hoveredNode = null
      invalidate()
    }
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault()
    cancelPanAnimation()
    const rect = canvas.getBoundingClientRect()
    const mouseX = e.clientX - rect.left
    const mouseY = e.clientY - rect.top
    const delta = -e.deltaY * 0.001
    const nextScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, scale + delta * scale))
    if (nextScale === scale) return

    panX = mouseX - (mouseX - panX) * (nextScale / scale)
    panY = mouseY - (mouseY - panY) * (nextScale / scale)
    scale = nextScale
    temporarilyHideDock()
    invalidate()
  }

  function closePopup() {
    selectedNode = null
    invalidate()
  }

  function retry() {
    fittedKey = null
    void refreshGitGraph(workspacePath, 300)
  }

  function resetView() {
    fitGraphToView()
    invalidate()
  }

  function screenForNode(node: CommitNode) {
    return {
      x: nodeWorldX(node) * scale + panX,
      y: nodeWorldY(node) * scale + panY,
    }
  }

  function clampOverlay(x: number, y: number, width: number, height: number) {
    const maxX = Math.max(4, (wrapper?.clientWidth ?? width) - width - 4)
    const maxY = Math.max(4, (wrapper?.clientHeight ?? height) - height - 4)
    return {
      x: Math.min(Math.max(4, x), maxX),
      y: Math.min(Math.max(4, y), maxY),
    }
  }

  function popupPosition(node: CommitNode) {
    const point = screenForNode(node)
    return clampOverlay(point.x + 14, point.y + 14, 380, 440)
  }
</script>

<div
  class="git-graph"
  class:dragging
  bind:this={wrapper}
>
  <canvas
    bind:this={canvas}
    aria-label="Git commit graph"
    onmousemove={onMouseMove}
    onmousedown={onMouseDown}
    onmouseup={onMouseUp}
    onmouseleave={onMouseLeave}
    onwheel={onWheel}
    ondblclick={resetView}
  ></canvas>

  {#if loading}
    <div class="graph-state">
      <span>{slowLoading ? 'Still reading commit graph...' : 'Loading Git history...'}</span>
    </div>
  {:else if error}
    <div class="graph-state error">
      <span>{error.includes('not a git repository') ? 'This workspace is not a Git repository.' : 'Unable to load Git history.'}</span>
      <details>
        <summary>Details</summary>
        <pre>{error}</pre>
      </details>
      <button type="button" onclick={retry}>Retry</button>
    </div>
  {:else if graphData && graphData.nodes.length === 0}
    <div class="graph-state">
      <span>No commits found in this repository.</span>
    </div>
  {:else if graphData?.truncated}
    <div class="truncated">Showing latest 300 commits</div>
  {/if}

  {#if graphData?.nodes.length && !loading && !error}
    <GitGraphDock
      nodes={nodes}
      branches={branches}
      visible={dockVisible}
      onGoto={focusNode}
    />
  {/if}

  {#if hoveredNode && !selectedNode && graphData?.nodes.length}
    <GitGraphTooltip node={hoveredNode} x={tooltipPos.x} y={tooltipPos.y} />
  {/if}

  {#if selectedNode && graphData?.nodes.length}
    {@const pos = popupPosition(selectedNode)}
    <GitGraphPopup
      node={selectedNode}
      workspacePath={workspacePath}
      x={pos.x}
      y={pos.y}
      onClose={closePopup}
    />
  {/if}
</div>

<style>
  .git-graph {
    position: relative;
    flex: 1;
    width: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background:
      linear-gradient(rgba(255, 255, 255, 0.025) 1px, transparent 1px),
      linear-gradient(90deg, rgba(255, 255, 255, 0.025) 1px, transparent 1px),
      var(--background);
    background-size: 28px 28px;
    cursor: grab;
  }
  .git-graph.dragging {
    cursor: grabbing;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
    touch-action: none;
  }
  .graph-state {
    position: absolute;
    inset: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 16px;
    text-align: center;
    color: var(--muted-foreground);
    background: color-mix(in srgb, var(--background) 78%, transparent);
    font-size: 12px;
  }
  .graph-state.error {
    color: var(--destructive);
  }
  .graph-state button {
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--secondary);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 12px;
    cursor: pointer;
  }
  .graph-state button:hover {
    border-color: var(--primary);
  }
  details {
    max-width: min(320px, calc(100% - 24px));
    color: var(--muted-foreground);
  }
  summary {
    cursor: pointer;
  }
  pre {
    max-height: 120px;
    overflow: auto;
    margin: 6px 0 0;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--popover);
    color: var(--foreground);
    text-align: left;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .truncated {
    position: absolute;
    top: 8px;
    right: 8px;
    z-index: 15;
    padding: 3px 7px;
    border: 1px solid color-mix(in srgb, var(--primary) 24%, transparent);
    border-radius: 6px;
    background: color-mix(in srgb, var(--popover) 88%, transparent);
    color: var(--muted-foreground);
    font-size: 10.5px;
    pointer-events: none;
  }
</style>
