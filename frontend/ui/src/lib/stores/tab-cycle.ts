/**
 * Ctrl+Tab MRU quick-switcher index math (M-keynav §4), extracted as a pure
 * helper so the cycle behavior is unit-testable away from App.svelte's keydown
 * shell.
 *
 * The quick switcher cycles through the live tab ids in MRU order. Forward
 * (Ctrl+Tab) starts at the *previous* tab (index 1) and steps forward; reverse
 * (Ctrl+Shift+Tab) starts at the last tab and steps backward. Both wrap. With 0
 * or 1 live tab there's nothing to switch to, so callers no-op.
 */

/** True when the switcher has something to do (more than one live tab). */
export function canSwitchTabs(liveIdCount: number): boolean {
  return liveIdCount > 1
}

/**
 * The starting index when the switcher first opens.
 *   - forward: the previous tab (1), so a single Ctrl+Tab flicks to it.
 *   - reverse: the last tab (count - 1).
 * Assumes `count > 1` (guard with `canSwitchTabs` first).
 */
export function initialSwitchIndex(count: number, reverse: boolean): number {
  return reverse ? count - 1 : 1
}

/** The next index while the switcher is held open, wrapping in `dir`. */
export function stepSwitchIndex(current: number, count: number, reverse: boolean): number {
  const dir = reverse ? -1 : 1
  return (current + dir + count) % count
}
