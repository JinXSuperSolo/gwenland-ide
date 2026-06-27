<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { get } from 'svelte/store'
  import type { UnlistenFn } from '@tauri-apps/api/event'
  import {
    terminalCreate,
    terminalWrite,
    terminalResize,
    terminalKill,
    onTerminalOutput,
  } from '../tauri/commands'
  import {
    createTerminal,
    fitTerminal,
    type TerminalBundle,
  } from '../terminal/xterm-setup'
  import { TerminalScheduler } from '../terminal/terminal-scheduler'
  import { bindPtyId, setDetectedPort, terminalSessions } from '../stores/terminal-sessions'
  import { workspace } from '../stores/workspace'
  import { editorPreferences } from '../stores/editor-preferences'
  import { perfSettings } from '../stores/performance'
  import { subscribeFocus, isAppActive } from '../stores/app-focus'
  import { registerTerminalHandle, unregisterTerminalHandle } from '../terminal/terminal-registry'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import { detectPreviewTarget } from '../terminal/port-detect'

  // One session per instance. `key` ties it to the sessions store; `visible`
  // drives the keep-alive show/hide (only the active tab is visible). All
  // instances stay mounted so scrollback + running processes are preserved.
  let { key, visible }: { key: string; visible: boolean } = $props()

  // M19 Wave 5: terminal minimap is the user's preference AND-ed with Low-End
  // Mode (which forces it off).
  const terminalMinimapEnabled = $derived(
    $editorPreferences.terminalMinimap && $perfSettings.showMinimap,
  )

  let host: HTMLDivElement
  let bundle: TerminalBundle | null = null
  let scheduler: TerminalScheduler | null = null
  let sessionId: string | null = null
  let unlisten: UnlistenFn | null = null
  let unsubscribeFocus: (() => void) | null = null
  let resizeObserver: ResizeObserver | null = null
  let disposed = false
  let minimapCanvas = $state<HTMLCanvasElement | null>(null)
  let minimapFrame = 0
  let minimapDragging = false
  let activityBins: number[] = []
  let terminalMinimapAccent = '#c28a64'
  let terminalMinimapColorCached = false
  const encoder = new TextEncoder()
  const decoder = new TextDecoder()
  let portBuffer = ''

  // M19 Wave 4: PTY output is fed to a frame-limiting scheduler (ring buffer +
  // rAF) so a flood (e.g. `cargo build`) coalesces into one repaint per frame
  // instead of one per chunk. The scheduler is also the pause point: it stops
  // writing while the window is backgrounded OR this tab is hidden, buffering a
  // bounded tail that's flushed in one write on resume.
  function renderOutput(bytes: Uint8Array) {
    const text = decoder.decode(bytes, { stream: true })
    recordTerminalActivity(bytes, text)
    if (text) {
      portBuffer = (portBuffer + text).slice(-4096)
      const preview = detectPreviewTarget(portBuffer)
      if (preview) setDetectedPort(key, preview.port, preview.url)
    }
    scheduler?.write(bytes)
  }

  /** Pause when hidden or backgrounded; resume (and flush) only when both the
   *  window is active AND this tab is visible. */
  function syncSchedulerState() {
    if (!scheduler) return
    if (visible && isAppActive()) scheduler.resume()
    else scheduler.pause()
  }

  function recordTerminalActivity(bytes: Uint8Array, text: string) {
    const count = Math.max(1, text.split(/\r?\n/).length)
    const weight = Math.min(1, Math.max(0.18, bytes.length / 900))
    for (let i = 0; i < count; i += 1) activityBins.push(weight)
    if (activityBins.length > 800) activityBins = activityBins.slice(-800)
    scheduleTerminalMinimapDraw()
  }

  function scheduleTerminalMinimapDraw() {
    // Skip canvas work when this terminal tab isn't visible — avoids background
    // rAF churn while another session tab is active.
    if (!visible || minimapFrame) return
    minimapFrame = requestAnimationFrame(() => {
      minimapFrame = 0
      drawTerminalMinimap()
    })
  }

  function drawTerminalMinimap() {
    if (!bundle || !minimapCanvas || !terminalMinimapEnabled) return
    const rect = minimapCanvas.getBoundingClientRect()
    if (rect.width <= 0 || rect.height <= 0) return
    const dpr = window.devicePixelRatio || 1
    minimapCanvas.width = Math.max(1, Math.floor(rect.width * dpr))
    minimapCanvas.height = Math.max(1, Math.floor(rect.height * dpr))
    const ctx = minimapCanvas.getContext('2d')
    if (!ctx) return
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, rect.width, rect.height)
    if (!terminalMinimapColorCached) {
      const a = getComputedStyle(minimapCanvas).getPropertyValue('--primary').trim()
      if (a) terminalMinimapAccent = a
      terminalMinimapColorCached = true
    }
    const accent = terminalMinimapAccent
    ctx.fillStyle = accent
    ctx.globalAlpha = 0.5
    const binHeight = Math.max(1, rect.height / Math.max(activityBins.length, 1))
    activityBins.forEach((weight, index) => {
      const y = index * binHeight
      ctx.fillRect(2, y, Math.max(2, (rect.width - 4) * weight), Math.max(1, binHeight * 0.8))
    })

    const term = bundle.term
    const buffer = term.buffer.active
    const total = Math.max(1, buffer.baseY + term.rows)
    const viewportY = buffer.viewportY ?? buffer.baseY
    const top = (viewportY / total) * rect.height
    const height = Math.max(12, (term.rows / total) * rect.height)
    ctx.globalAlpha = 1
    ctx.strokeStyle = accent
    ctx.strokeRect(1.5, top + 0.5, rect.width - 3, height - 1)
  }

  function scrollTerminalFromMinimap(e: PointerEvent) {
    if (!bundle || !minimapCanvas) return
    const rect = minimapCanvas.getBoundingClientRect()
    const ratio = Math.min(1, Math.max(0, (e.clientY - rect.top) / Math.max(rect.height, 1)))
    const buffer = bundle.term.buffer.active
    const total = Math.max(1, buffer.baseY + bundle.term.rows)
    bundle.term.scrollToLine(Math.floor(ratio * total))
    scheduleTerminalMinimapDraw()
  }

  function startMinimapDrag(e: PointerEvent) {
    minimapDragging = true
    minimapCanvas?.setPointerCapture(e.pointerId)
    scrollTerminalFromMinimap(e)
  }

  function dragMinimap(e: PointerEvent) {
    if (minimapDragging) scrollTerminalFromMinimap(e)
  }

  function stopMinimapDrag(e: PointerEvent) {
    minimapDragging = false
    minimapCanvas?.releasePointerCapture(e.pointerId)
  }

  function refit() {
    if (!bundle || !sessionId) return
    const dims = fitTerminal(bundle)
    if (dims) void terminalResize(sessionId, dims.rows, dims.cols).catch(() => {})
    scheduleTerminalMinimapDraw()
  }

  // Becoming visible after being hidden: an element with display:none can't be
  // measured, so xterm couldn't fit while hidden. Re-fit + refocus on show.
  // Also resume/pause the output scheduler off the visibility change.
  $effect(() => {
    // Reference `visible` so this effect re-runs on tab show/hide.
    void visible
    syncSchedulerState()
    if (visible && bundle && sessionId) {
      // Defer to let the DOM apply the visibility change before measuring.
      requestAnimationFrame(() => {
        refit()
        bundle?.term.focus()
      })
    }
  })

  $effect(() => {
    void terminalMinimapEnabled
    scheduleTerminalMinimapDraw()
  })

  // Expose this terminal to the M9 context-menu actions (Copy/Paste/Clear/
  // Select All act on the specific instance the menu was opened over).
  function registerHandle() {
    registerTerminalHandle(key, {
      getSelection: () => bundle?.term.getSelection() ?? '',
      copySelection: async () => {
        const sel = bundle?.term.getSelection()
        if (!sel) return
        try {
          await navigator.clipboard.writeText(sel)
        } catch {
          /* clipboard unavailable */
        }
      },
      paste: async () => {
        if (!sessionId) return
        let text = ''
        try {
          text = await navigator.clipboard.readText()
        } catch {
          return
        }
        if (text) void terminalWrite(sessionId, encoder.encode(text)).catch(() => {})
      },
      clear: () => bundle?.term.clear(),
      selectAll: () => bundle?.term.selectAll(),
      focus: () => bundle?.term.focus(),
      readBuffer: (maxLines: number) => {
        const term = bundle?.term
        if (!term) return ''
        // Read the active buffer bottom-up so we keep the most recent lines,
        // including scrollback above the viewport (baseY + visible rows).
        const buf = term.buffer.active
        const last = buf.baseY + term.rows
        const lines: string[] = []
        for (let y = last - 1; y >= 0 && lines.length < maxLines; y--) {
          const line = buf.getLine(y)
          if (line) lines.unshift(line.translateToString(true))
        }
        // Drop leading/trailing blank lines so the injected block is compact.
        return lines.join('\n').replace(/^\s*\n/, '').replace(/\n\s*$/, '')
      },
    })
  }

  // Right-click opens the terminal context menu (M9), carrying this session's
  // key and current selection so selection-aware actions gate correctly.
  function onTermContextMenu(e: MouseEvent) {
    openContextMenu(e, {
      scope: 'terminal',
      terminalId: key,
      terminalSelection: bundle?.term.getSelection() || undefined,
    })
  }

  onMount(() => {
    bundle = createTerminal(host)
    scheduler = new TerminalScheduler(bundle.term)
    // Seed the scheduler's pause state from current visibility/focus.
    syncSchedulerState()
    registerHandle()
    const { rows, cols } = bundle.term

    ;(async () => {
      try {
        // Start in the session's explicit cwd ("Open in Terminal"), else the
        // current project folder (snapshot at spawn time).
        const session = get(terminalSessions).sessions.find((s) => s.key === key)
        const cwd = session?.cwd ?? get(workspace).folderPath
        const id = await terminalCreate(rows, cols, cwd, session?.shellCommand ?? null)
        if (disposed) {
          void terminalKill(id) // torn down mid-spawn; don't leak the PTY.
          return
        }
        sessionId = id
        bindPtyId(key, id)

        // Render via the throttle gate: written immediately while active,
        // buffered while the window is in the background (PTY keeps streaming).
        unlisten = await onTerminalOutput(id, (bytes) => renderOutput(bytes))
        bundle!.term.onData((data: string) => {
          void terminalWrite(id, encoder.encode(data)).catch(() => {})
        })

        if (visible) bundle!.term.focus()
        refit()
      } catch (err) {
        bundle?.term.writeln(`\x1b[31mFailed to start terminal: ${err}\x1b[0m`)
      }
    })()

    resizeObserver = new ResizeObserver(() => refit())
    resizeObserver.observe(host)

    // On focus change, resume/pause the scheduler (resume flushes whatever
    // streamed in while backgrounded) and re-fit on focus (size may have
    // changed). The PTY never stopped, so resume catches up in one repaint.
    unsubscribeFocus = subscribeFocus((active) => {
      syncSchedulerState()
      if (active && bundle && visible) refit()
    })

    return () => {
      disposed = true
    }
  })

  onDestroy(() => {
    unregisterTerminalHandle(key)
    resizeObserver?.disconnect()
    resizeObserver = null
    unsubscribeFocus?.()
    unsubscribeFocus = null
    scheduler?.dispose()
    scheduler = null
    if (unlisten) {
      unlisten()
      unlisten = null
    }
    if (sessionId) {
      void terminalKill(sessionId)
      sessionId = null
    }
    bundle?.term.dispose()
    bundle = null
  })
</script>

<!-- One terminal viewport. Hidden (not unmounted) when its tab is inactive. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="term-instance"
  class:with-minimap={terminalMinimapEnabled}
  class:hidden={!visible}
  bind:this={host}
  oncontextmenu={onTermContextMenu}
>
  {#if terminalMinimapEnabled}
    <canvas
      bind:this={minimapCanvas}
      class="terminal-minimap"
      aria-label="Terminal minimap"
      onpointerdown={startMinimapDrag}
      onpointermove={dragMinimap}
      onpointerup={stopMinimapDrag}
      onpointercancel={stopMinimapDrag}
    ></canvas>
  {/if}
</div>

<style>
  .term-instance {
    height: 100%;
    width: 100%;
    overflow: hidden;
    position: relative;
  }
  .term-instance.with-minimap :global(.xterm) {
    width: calc(100% - 14px);
  }
  .terminal-minimap {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 4;
    width: 12px;
    border-left: 1px solid var(--border);
    background: color-mix(in srgb, var(--background) 86%, #1c1c1c);
    cursor: pointer;
    touch-action: none;
  }
  .term-instance.hidden {
    display: none;
  }
  .term-instance :global(.xterm) {
    height: 100%;
  }
  .term-instance :global(.xterm-viewport) {
    background-color: transparent !important;
  }
</style>
