import { writable, get } from 'svelte/store'

/**
 * App focus/visibility controller (background throttle).
 *
 * The window is "active" when it is BOTH focused and visible. When it goes
 * inactive (blur or tab hidden / minimized), background work pauses to cut idle
 * CPU/RAM; when it becomes active again, work resumes and a few things refresh
 * immediately.
 *
 * What pauses: git polling, terminal (XTerm) repainting, other non-critical
 * timers. What does NOT pause: the PTY process itself (it keeps running in the
 * background), an active AI streaming response, and any filesystem watchers.
 *
 * Consumers either read the `appActive` store reactively or register an
 * `onFocusChange(active)` listener via `subscribeFocus` and run resume work
 * (e.g. an immediate git refresh) on the false -> true edge.
 */

/** True when the window is focused AND the document is visible. */
export const appActive = writable<boolean>(true)

type FocusListener = (active: boolean) => void
const listeners = new Set<FocusListener>()

/**
 * Native (Tauri) window focus, when available. In the WebView2/desktop shell the
 * OS-level focus event is more reliable than DOM blur for minimize/restore, so
 * we AND it in. `null` until the first native event arrives (DOM signals carry
 * the state until then).
 */
let nativeFocused: boolean | null = null

/** Compute current active-ness from the platform. SSR-safe (defaults true). */
function computeActive(): boolean {
  if (typeof document === 'undefined') return true
  // `document.hasFocus()` covers blur (another app/window focused); the
  // visibility state covers a hidden/minimized tab or window.
  const visible = document.visibilityState !== 'hidden'
  const focused = document.hasFocus()
  const dom = visible && focused
  // Fold in the native focus signal once we have one.
  return nativeFocused === null ? dom : dom && nativeFocused
}

let started = false

/**
 * Begin listening for focus/visibility changes. Idempotent. Call once at
 * startup. Emits to `appActive` and to every `subscribeFocus` listener only on a
 * real transition (so resume work doesn't fire spuriously).
 */
export function initAppFocus(): void {
  if (started || typeof window === 'undefined') return
  started = true

  const update = () => {
    const next = computeActive()
    if (get(appActive) === next) return // only act on a real transition
    appActive.set(next)
    for (const fn of listeners) {
      try {
        fn(next)
      } catch {
        /* a misbehaving listener must not break the others */
      }
    }
  }

  // blur/focus catches another app taking focus; visibilitychange catches a
  // minimized/hidden window. Together they cover every "no longer in front" case.
  window.addEventListener('blur', update)
  window.addEventListener('focus', update)
  document.addEventListener('visibilitychange', update)

  // Supplement with Tauri's native window focus event (more reliable for
  // minimize/restore in the desktop shell). Best-effort: if the API isn't
  // present (e.g. plain browser dev), we silently rely on the DOM events.
  void import('@tauri-apps/api/window')
    .then(({ getCurrentWindow }) =>
      getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        nativeFocused = focused
        update()
      }),
    )
    .catch(() => {
      /* not running under Tauri — DOM events suffice */
    })

  appActive.set(computeActive())
}

/**
 * Register a transition listener. `fn(active)` runs on every false<->true flip
 * (after `appActive` is updated). Returns an unsubscribe function.
 */
export function subscribeFocus(fn: FocusListener): () => void {
  listeners.add(fn)
  return () => listeners.delete(fn)
}

/** Synchronous read of the current active state. */
export function isAppActive(): boolean {
  return get(appActive)
}
