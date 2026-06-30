/**
 * Centralized Escape / overlay stack (M-keynav §2).
 *
 * The app's modals, popovers, and dialogs each own their open state in their own
 * store (command palette, context menu, prompt, settings, About, changelog, git
 * graph, …). Before this, every one of them listened for Escape independently on
 * `svelte:window`, so a single Escape could close *several* stacked overlays at
 * once (every listener fired). This module makes Escape deterministic:
 *
 *   - Each overlay registers a `close` handler once, and reports when it opens /
 *     closes so the stack knows the live z-order (open order = stack order).
 *   - `closeTopmost()` closes exactly ONE overlay — the most-recently-opened one
 *     — and returns whether it closed anything. App-level Escape calls this, so
 *     one press peels one layer; nothing open is a no-op.
 *
 * The stack is intentionally tiny and dependency-free. Open order is tracked with
 * a monotonic counter rather than array position so re-opening an overlay moves
 * it to the top without needing the caller to dedupe.
 */

export interface OverlayEntry {
  /** Stable identifier for the overlay surface. */
  id: string
  /** Close the overlay. Must be idempotent (safe if already closed). */
  close: () => void
}

interface LiveOverlay extends OverlayEntry {
  /** Monotonic open sequence; higher = opened more recently = closer to top. */
  seq: number
}

const registry = new Map<string, OverlayEntry>()
const open = new Map<string, LiveOverlay>()
let counter = 0

/**
 * Register an overlay's close handler. Call once (e.g. on component mount).
 * Re-registering with the same id replaces the handler. Returns an unregister
 * function that also drops it from the open set.
 */
export function registerOverlay(entry: OverlayEntry): () => void {
  registry.set(entry.id, entry)
  return () => {
    registry.delete(entry.id)
    open.delete(entry.id)
  }
}

/** Mark an overlay open (push to / refresh at the top of the stack). */
export function markOverlayOpen(id: string): void {
  const entry = registry.get(id)
  if (!entry) return
  open.set(id, { ...entry, seq: ++counter })
}

/** Mark an overlay closed (remove from the stack). No-op if not open. */
export function markOverlayClosed(id: string): void {
  open.delete(id)
}

/**
 * Sync an overlay's open/closed membership from a boolean. Convenience for the
 * common pattern of mirroring a store's `open` flag via an `$effect`.
 */
export function syncOverlay(id: string, isOpen: boolean): void {
  if (isOpen) markOverlayOpen(id)
  else markOverlayClosed(id)
}

/** The id of the topmost (most-recently-opened) open overlay, or null. */
export function topmostOverlayId(): string | null {
  let top: LiveOverlay | null = null
  for (const entry of open.values()) {
    if (!top || entry.seq > top.seq) top = entry
  }
  return top?.id ?? null
}

/** Number of overlays currently open (for tests / diagnostics). */
export function openOverlayCount(): number {
  return open.size
}

/**
 * Close exactly the topmost open overlay. Returns true if one was closed, false
 * if nothing was open (so the caller can let Escape fall through as a no-op).
 */
export function closeTopmost(): boolean {
  const id = topmostOverlayId()
  if (!id) return false
  const entry = open.get(id)
  open.delete(id)
  entry?.close()
  return true
}

/** Test helper: drop all registrations and open state. */
export function resetOverlayStack(): void {
  registry.clear()
  open.clear()
  counter = 0
}
