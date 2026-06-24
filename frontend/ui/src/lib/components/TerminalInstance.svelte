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
  import { bindPtyId, terminalSessions } from '../stores/terminal-sessions'
  import { workspace } from '../stores/workspace'
  import { registerTerminalHandle, unregisterTerminalHandle } from '../terminal/terminal-registry'
  import { openContextMenu } from '../context-menu/contextMenuStore'

  // One session per instance. `key` ties it to the sessions store; `visible`
  // drives the keep-alive show/hide (only the active tab is visible). All
  // instances stay mounted so scrollback + running processes are preserved.
  let { key, visible }: { key: string; visible: boolean } = $props()

  let host: HTMLDivElement
  let bundle: TerminalBundle | null = null
  let sessionId: string | null = null
  let unlisten: UnlistenFn | null = null
  let resizeObserver: ResizeObserver | null = null
  let disposed = false
  const encoder = new TextEncoder()

  function refit() {
    if (!bundle || !sessionId) return
    const dims = fitTerminal(bundle)
    if (dims) void terminalResize(sessionId, dims.rows, dims.cols).catch(() => {})
  }

  // Becoming visible after being hidden: an element with display:none can't be
  // measured, so xterm couldn't fit while hidden. Re-fit + refocus on show.
  $effect(() => {
    if (visible && bundle && sessionId) {
      // Defer to let the DOM apply the visibility change before measuring.
      requestAnimationFrame(() => {
        refit()
        bundle?.term.focus()
      })
    }
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
    registerHandle()
    const { rows, cols } = bundle.term

    ;(async () => {
      try {
        // Start in the session's explicit cwd ("Open in Terminal"), else the
        // current project folder (snapshot at spawn time).
        const session = get(terminalSessions).sessions.find((s) => s.key === key)
        const cwd = session?.cwd ?? get(workspace).folderPath
        const id = await terminalCreate(rows, cols, cwd)
        if (disposed) {
          void terminalKill(id) // torn down mid-spawn; don't leak the PTY.
          return
        }
        sessionId = id
        bindPtyId(key, id)

        unlisten = await onTerminalOutput(id, (bytes) => bundle?.term.write(bytes))
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

    return () => {
      disposed = true
    }
  })

  onDestroy(() => {
    unregisterTerminalHandle(key)
    resizeObserver?.disconnect()
    resizeObserver = null
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
  class:hidden={!visible}
  bind:this={host}
  oncontextmenu={onTermContextMenu}
></div>

<style>
  .term-instance {
    height: 100%;
    width: 100%;
    overflow: hidden;
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
