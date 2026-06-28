<script lang="ts">
  import {
    closeGitGraphWindow,
    gitGraphWindow,
    setGitGraphWindowBounds,
    toggleGitGraphWindowMaximized,
  } from '../../stores/gitGraphWindow'
  import { refreshGitGraph } from '../../stores/gitGraph'
  import Icon from '../Icon.svelte'
  import GitGraph from './GitGraph.svelte'

  const MIN_WIDTH = 520
  const MIN_HEIGHT = 340
  const VIEWPORT_MARGIN = 8

  type DragState =
    | {
        mode: 'move'
        startX: number
        startY: number
        x: number
        y: number
      }
    | {
        mode: 'resize'
        startX: number
        startY: number
        width: number
        height: number
      }

  let drag = $state<DragState | null>(null)

  const windowTitle = $derived(
    $gitGraphWindow.workspacePath
      ? `Git Graph - ${$gitGraphWindow.workspacePath}`
      : 'Git Graph',
  )
  const frameStyle = $derived.by(() => {
    if ($gitGraphWindow.maximized) {
      return 'left: 12px; top: 42px; width: calc(100vw - 24px); height: calc(100vh - 72px);'
    }
    return [
      `left: ${$gitGraphWindow.x}px`,
      `top: ${$gitGraphWindow.y}px`,
      `width: ${$gitGraphWindow.width}px`,
      `height: ${$gitGraphWindow.height}px`,
    ].join('; ')
  })

  function viewportWidth(): number {
    return Math.max(MIN_WIDTH, window.innerWidth || MIN_WIDTH)
  }

  function viewportHeight(): number {
    return Math.max(MIN_HEIGHT, window.innerHeight || MIN_HEIGHT)
  }

  function clampBounds(x: number, y: number, width: number, height: number) {
    const maxWidth = Math.max(MIN_WIDTH, viewportWidth() - VIEWPORT_MARGIN * 2)
    const maxHeight = Math.max(MIN_HEIGHT, viewportHeight() - VIEWPORT_MARGIN * 2)
    const nextWidth = Math.min(Math.max(MIN_WIDTH, width), maxWidth)
    const nextHeight = Math.min(Math.max(MIN_HEIGHT, height), maxHeight)
    const nextX = Math.min(
      Math.max(VIEWPORT_MARGIN, x),
      Math.max(VIEWPORT_MARGIN, viewportWidth() - nextWidth - VIEWPORT_MARGIN),
    )
    const nextY = Math.min(
      Math.max(VIEWPORT_MARGIN, y),
      Math.max(VIEWPORT_MARGIN, viewportHeight() - nextHeight - VIEWPORT_MARGIN),
    )
    return { x: nextX, y: nextY, width: nextWidth, height: nextHeight }
  }

  function startMove(e: PointerEvent): void {
    if ($gitGraphWindow.maximized) return
    drag = {
      mode: 'move',
      startX: e.clientX,
      startY: e.clientY,
      x: $gitGraphWindow.x,
      y: $gitGraphWindow.y,
    }
    e.preventDefault()
  }

  function startResize(e: PointerEvent): void {
    if ($gitGraphWindow.maximized) return
    drag = {
      mode: 'resize',
      startX: e.clientX,
      startY: e.clientY,
      width: $gitGraphWindow.width,
      height: $gitGraphWindow.height,
    }
    e.preventDefault()
    e.stopPropagation()
  }

  function onPointerMove(e: PointerEvent): void {
    if (!drag || $gitGraphWindow.maximized) return
    if (drag.mode === 'move') {
      const next = clampBounds(
        drag.x + e.clientX - drag.startX,
        drag.y + e.clientY - drag.startY,
        $gitGraphWindow.width,
        $gitGraphWindow.height,
      )
      setGitGraphWindowBounds({ x: next.x, y: next.y })
      return
    }

    const next = clampBounds(
      $gitGraphWindow.x,
      $gitGraphWindow.y,
      drag.width + e.clientX - drag.startX,
      drag.height + e.clientY - drag.startY,
    )
    setGitGraphWindowBounds(next)
  }

  function stopDrag(): void {
    drag = null
  }

  function refresh(): void {
    if ($gitGraphWindow.workspacePath) {
      void refreshGitGraph($gitGraphWindow.workspacePath)
    }
  }
</script>

<svelte:window onpointermove={onPointerMove} onpointerup={stopDrag} />

{#if $gitGraphWindow.open && $gitGraphWindow.workspacePath}
  <section
    class="git-graph-window"
    class:maximized={$gitGraphWindow.maximized}
    class:dragging={drag?.mode === 'move'}
    style={frameStyle}
    aria-label="Git Graph floating window"
  >
    <header
      class="window-titlebar"
      role="group"
      aria-label="Git Graph window title bar"
      onpointerdown={startMove}
    >
      <div class="title-main">
        <Icon name="git-branch" size={15} />
        <div class="title-copy">
          <span class="title">Git Graph</span>
          <span class="workspace" title={$gitGraphWindow.workspacePath}>{$gitGraphWindow.workspacePath}</span>
        </div>
      </div>
      <div class="window-actions">
        <button
          type="button"
          title="Refresh Git Graph"
          aria-label="Refresh Git Graph"
          onpointerdown={(e) => e.stopPropagation()}
          onclick={refresh}
        >
          <Icon name="refresh" size={14} />
        </button>
        <button
          type="button"
          title={$gitGraphWindow.maximized ? 'Restore Git Graph' : 'Maximize Git Graph'}
          aria-label={$gitGraphWindow.maximized ? 'Restore Git Graph' : 'Maximize Git Graph'}
          onpointerdown={(e) => e.stopPropagation()}
          onclick={toggleGitGraphWindowMaximized}
        >
          <Icon name={$gitGraphWindow.maximized ? 'collapse' : 'open-in-window'} size={14} />
        </button>
        <button
          type="button"
          title="Close Git Graph"
          aria-label="Close Git Graph"
          onpointerdown={(e) => e.stopPropagation()}
          onclick={closeGitGraphWindow}
        >
          <Icon name="xmark" size={14} />
        </button>
      </div>
    </header>

    <div class="window-body" aria-label={windowTitle}>
      <GitGraph workspacePath={$gitGraphWindow.workspacePath} />
    </div>

    {#if !$gitGraphWindow.maximized}
      <button
        type="button"
        class="resize-corner"
        aria-label="Resize Git Graph"
        onpointerdown={startResize}
      ></button>
    {/if}
  </section>
{/if}

<style>
  .git-graph-window {
    position: fixed;
    z-index: 88;
    min-width: 520px;
    min-height: 340px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--primary) 26%, var(--border));
    border-radius: 7px;
    background: color-mix(in srgb, var(--background) 98%, black);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.42), 0 0 0 1px rgba(255, 255, 255, 0.025);
  }
  .git-graph-window.maximized {
    border-radius: var(--radius-sm);
  }
  .window-titlebar {
    height: 36px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 8px 0 11px;
    border-bottom: 1px solid var(--border);
    background:
      linear-gradient(90deg, color-mix(in srgb, var(--primary) 15%, transparent), transparent 42%),
      color-mix(in srgb, var(--popover) 90%, var(--background));
    cursor: grab;
    user-select: none;
  }
  .git-graph-window.dragging .window-titlebar {
    cursor: grabbing;
  }
  .title-main {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--primary);
  }
  .title-copy {
    min-width: 0;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: baseline;
    gap: 8px;
  }
  .title {
    color: var(--foreground);
    font-size: 12px;
    font-weight: 800;
  }
  .workspace {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 10.5px;
  }
  .window-actions {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .window-actions button {
    width: 26px;
    height: 26px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
  }
  .window-actions button:hover {
    color: var(--foreground);
    background: var(--secondary);
  }
  .window-body {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }
  .resize-corner {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    background:
      linear-gradient(135deg, transparent 0 46%, color-mix(in srgb, var(--primary) 46%, transparent) 47% 53%, transparent 54%),
      linear-gradient(135deg, transparent 0 66%, color-mix(in srgb, var(--primary) 32%, transparent) 67% 73%, transparent 74%);
    cursor: nwse-resize;
  }
  @media (max-width: 760px) {
    .git-graph-window {
      left: 8px !important;
      top: 42px !important;
      width: calc(100vw - 16px) !important;
      height: calc(100vh - 72px) !important;
      min-width: 0;
      min-height: 280px;
    }
    .title-copy {
      grid-template-columns: minmax(0, 1fr);
      gap: 0;
    }
    .workspace {
      display: none;
    }
  }
</style>
