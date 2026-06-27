import { get } from 'svelte/store'
import { afterEach, describe, expect, it } from 'vitest'
import { perfSettings } from './performance'
import { editorPreferences, setLowEndMode } from './editor-preferences'

/** Low-End Mode effect map (M19 Wave 5). */
describe('perfSettings', () => {
  afterEach(() => setLowEndMode(false))

  it('enables every feature when low-end mode is off', () => {
    setLowEndMode(false)
    const p = get(perfSettings)
    expect(p).toEqual({
      showGitBadges: true,
      showIndentGuides: true,
      smoothScroll: true,
      showMinimap: true,
      stickyScroll: true,
      animations: true,
      showFileIcons: true,
    })
  })

  it('disables every feature when low-end mode is on', () => {
    setLowEndMode(true)
    const p = get(perfSettings)
    expect(Object.values(p).every((v) => v === false)).toBe(true)
  })

  it('reacts to toggling the underlying preference', () => {
    setLowEndMode(false)
    expect(get(perfSettings).showMinimap).toBe(true)
    setLowEndMode(true)
    expect(get(perfSettings).showMinimap).toBe(false)
    expect(get(editorPreferences).lowEndMode).toBe(true)
  })
})
