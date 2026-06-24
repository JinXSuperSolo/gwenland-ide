// XTerm.js setup (Milestone 3, Wave 3). Mirrors the editor's codemirror-setup.ts
// pattern: this module owns terminal construction + addon wiring + theme so the
// Terminal.svelte component stays thin.
//
// Rendering: Canvas 2D via @xterm/addon-canvas — a locked M3 decision (NOT
// WebGL, NOT the default DOM renderer). The fit addon recomputes cols/rows from
// the host element's pixel size; the panel-resize hook in Terminal.svelte calls
// fit() and reports the new dimensions back to the PTY.

import { Terminal, type ITheme } from '@xterm/xterm'
import { CanvasAddon } from '@xterm/addon-canvas'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'

/**
 * Terminal colour theme aligned with the IDE's dark tokens (tokens.css). XTerm
 * paints to a canvas and cannot read CSS `oklch` custom properties, so these are
 * concrete values approximating the design palette (background = --background,
 * foreground = --foreground, cursor/selection = --primary).
 */
const IDE_THEME: ITheme = {
  background: '#1c1c1c',
  foreground: '#fafafa',
  cursor: '#c58e63',
  cursorAccent: '#1c1c1c',
  selectionBackground: 'rgba(197, 142, 99, 0.32)',
  // A standard 16-colour ANSI set tuned to read well on the dark background.
  black: '#1c1c1c',
  red: '#e06c75',
  green: '#98c379',
  yellow: '#e5c07b',
  blue: '#61afef',
  magenta: '#c678dd',
  cyan: '#56b6c2',
  white: '#dcdfe4',
  brightBlack: '#5c6370',
  brightRed: '#e06c75',
  brightGreen: '#98c379',
  brightYellow: '#e5c07b',
  brightBlue: '#61afef',
  brightMagenta: '#c678dd',
  brightCyan: '#56b6c2',
  brightWhite: '#ffffff',
}

/** A terminal instance plus the addons we need handles to (for fit/dispose). */
export interface TerminalBundle {
  term: Terminal
  fit: FitAddon
}

/**
 * Creates an XTerm terminal with the Canvas 2D renderer and fit addon, opened on
 * `host`. The caller is responsible for wiring `term.onData` to the PTY and
 * writing PTY output back via `term.write`, and for calling `bundle.term.dispose()`
 * on teardown.
 */
export function createTerminal(host: HTMLElement): TerminalBundle {
  const term = new Terminal({
    theme: IDE_THEME,
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 13,
    lineHeight: 1.2,
    cursorBlink: true,
    // Generous client-side scrollback for usability; the Rust ring buffer
    // (Wave 5) caps server-side retention separately.
    scrollback: 5000,
    // Lets the host element drive size; fit() computes cols/rows from pixels.
    allowProposedApi: true,
  })

  // Order matters: open() must run before addons that measure the DOM.
  term.open(host)
  term.loadAddon(new CanvasAddon())
  const fit = new FitAddon()
  term.loadAddon(fit)
  fit.fit()

  return { term, fit }
}

/**
 * Re-fits the terminal to its host's current size and returns the resulting
 * dimensions, or null if the element isn't measurable yet (e.g. collapsed).
 * Terminal.svelte forwards these to `terminalResize` so the PTY agrees.
 */
export function fitTerminal(bundle: TerminalBundle): { rows: number; cols: number } | null {
  try {
    bundle.fit.fit()
  } catch {
    return null
  }
  const { rows, cols } = bundle.term
  if (!rows || !cols) return null
  return { rows, cols }
}
