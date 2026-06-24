import { writable } from 'svelte/store'

/**
 * Layout state for the two collapsible panels:
 *   - File Tree: docked left, resized HORIZONTALLY (a `width` in px).
 *   - Terminal: docked below the Workspace, resized VERTICALLY (a `height` in
 *     px) — matching the VS Code / Zed / JetBrains convention of a bottom panel.
 *
 * The Workspace is never collapsible — it takes the remaining space in both
 * axes — so it has no entry here.
 *
 * The resizable dimension is stored in px. When a drag shrinks a panel below
 * COLLAPSE_THRESHOLD it auto-collapses: `collapsed` flips true and the panel is
 * removed from the layout. The dimension retains its last expanded value so
 * restore can return to it.
 */
export interface PanelState {
  /** Last expanded size in px along the panel's resize axis (preserved for restore). */
  size: number
  collapsed: boolean
}

export interface PanelsState {
  /** File Tree — `size` is its width (horizontal resize). */
  fileTree: PanelState
  /** Terminal — `size` is its height (vertical resize). */
  terminal: PanelState
  /** AI panel — docked right; `size` is its width. Visibility is driven by
   *  `aiChat.isOpen` (the status-bar toggle); `size` is reused for resizing. */
  aiPanel: PanelState
}

/** Drag below this many px (along the resize axis) snaps the panel to collapsed. */
export const COLLAPSE_THRESHOLD = 40

/** Per-panel resize bounds (px along the panel's resize axis). A drag is clamped
 *  to `[min, max]` while expanded, so no panel can be dragged uselessly small or
 *  large enough to swallow the workspace. (`ResizeHandle` additionally caps to a
 *  fraction of the window so the limits stay sane on small windows.) */
export interface PanelLimits {
  min: number
  max: number
}
export const PANEL_LIMITS: Record<PanelKey, PanelLimits> = {
  fileTree: { min: 160, max: 560 },
  terminal: { min: 100, max: 720 },
  aiPanel: { min: 300, max: 680 },
}

/** Default sizes used on first load and when restoring from collapsed. */
export const DEFAULT_FILE_TREE_WIDTH = 260
export const DEFAULT_TERMINAL_HEIGHT = 200
export const DEFAULT_AI_PANEL_WIDTH = 380

const initial: PanelsState = {
  fileTree: { size: DEFAULT_FILE_TREE_WIDTH, collapsed: false },
  terminal: { size: DEFAULT_TERMINAL_HEIGHT, collapsed: false },
  // Collapsed by default; the status-bar AI button opens it via aiChat.isOpen.
  aiPanel: { size: DEFAULT_AI_PANEL_WIDTH, collapsed: true },
}

export const panels = writable<PanelsState>(initial)

export type PanelKey = keyof PanelsState

/**
 * Apply a drag-resize result for a panel. Below COLLAPSE_THRESHOLD the panel
 * auto-collapses (keeping the previous expanded size so restore works);
 * otherwise the size is clamped to the panel's `[min, max]`. Axis-agnostic:
 * callers pass a width for File Tree / AI panel or a height for Terminal.
 */
export function setPanelSize(key: PanelKey, nextSize: number): void {
  panels.update((state) => {
    const panel = state[key]
    if (nextSize < COLLAPSE_THRESHOLD) {
      // Snap to collapsed but DON'T overwrite size — keep last good size.
      return { ...state, [key]: { ...panel, collapsed: true } }
    }
    const { min, max } = PANEL_LIMITS[key]
    return {
      ...state,
      [key]: { size: Math.min(Math.max(nextSize, min), max), collapsed: false },
    }
  })
}

/** Restore a collapsed panel to its remembered size. */
export function expandPanel(key: PanelKey): void {
  panels.update((state) => ({
    ...state,
    [key]: { ...state[key], collapsed: false },
  }))
}

/** Explicitly collapse a panel (e.g. a header toggle), keeping its size. */
export function collapsePanel(key: PanelKey): void {
  panels.update((state) => ({
    ...state,
    [key]: { ...state[key], collapsed: true },
  }))
}

/** Toggle a panel's collapsed state (menu/shortcut use). */
export function togglePanel(key: PanelKey): void {
  panels.update((state) => ({
    ...state,
    [key]: { ...state[key], collapsed: !state[key].collapsed },
  }))
}
