import { derived } from 'svelte/store'
import { editorPreferences } from './editor-preferences'

/**
 * Derived performance feature flags (M19 Wave 5, GWEN-379).
 *
 * Low-End Mode disables expensive visual features for smoother performance on
 * older hardware. Components that own a feature read the matching flag from here
 * instead of checking `lowEndMode` directly, so the effect map lives in one
 * place. Features that already have their own opt-in (minimap, markdown preview)
 * are additionally forced OFF when low-end mode is on.
 *
 * Globally-applied effects (animations, indent guides, file icons) are handled
 * with a `low-end` class on <body> + CSS overrides rather than per-component
 * flags — see `applyLowEndClass`.
 */
export interface PerfSettings {
  showGitBadges: boolean
  showIndentGuides: boolean
  smoothScroll: boolean
  showMinimap: boolean
  stickyScroll: boolean
  animations: boolean
  showFileIcons: boolean
}

export const perfSettings = derived<typeof editorPreferences, PerfSettings>(
  editorPreferences,
  ($p) => {
    const low = $p.lowEndMode
    return {
      showGitBadges: !low,
      showIndentGuides: !low,
      smoothScroll: !low,
      showMinimap: !low,
      stickyScroll: !low,
      animations: !low,
      showFileIcons: !low,
    }
  },
)

/**
 * Toggle a `low-end` class on <body> so global CSS overrides (in base.css /
 * animations.css) can strip animations, indent guides, and file icons without
 * threading a flag through dozens of components. Call once at startup; it keeps
 * the class in sync with the store.
 */
export function applyLowEndClass(): void {
  if (typeof document === 'undefined') return
  editorPreferences.subscribe(($p) => {
    document.body.classList.toggle('low-end', $p.lowEndMode)
  })
}
