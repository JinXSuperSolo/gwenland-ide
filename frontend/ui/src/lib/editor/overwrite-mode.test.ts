import { get } from 'svelte/store'
import { afterEach, describe, expect, it } from 'vitest'
import { overwriteMode, setOverwriteMode, syncOverwriteFromView } from './overwrite-mode'

/**
 * Behavioral tests for the overwrite-mode safety net (M-keynav §5/§6). The store
 * is hidden (false) by default; it surfaces the OVR status-bar indicator only if
 * overwrite mode is somehow activated. The Insert key is hard-blocked elsewhere
 * (blockInsertKeymap), so under normal use this stays false.
 */

afterEach(() => setOverwriteMode(false))

describe('overwriteMode store', () => {
  it('is hidden (false) by default', () => {
    expect(get(overwriteMode)).toBe(false)
  })

  it('surfaces when forced on, hides when forced off', () => {
    setOverwriteMode(true)
    expect(get(overwriteMode)).toBe(true)
    setOverwriteMode(false)
    expect(get(overwriteMode)).toBe(false)
  })
})

describe('syncOverwriteFromView', () => {
  it('detects an internal overwrite flag and mirrors it', () => {
    expect(syncOverwriteFromView({ inputState: { overwrite: true } })).toBe(true)
    expect(get(overwriteMode)).toBe(true)
  })

  it('detects a contentDOM overwrite dataset marker', () => {
    const view = { contentDOM: { dataset: { overwrite: 'true' } } }
    expect(syncOverwriteFromView(view)).toBe(true)
    expect(get(overwriteMode)).toBe(true)
  })

  it('reports false (and stays hidden) for a normal view with no overwrite', () => {
    expect(syncOverwriteFromView({ inputState: { overwrite: false } })).toBe(false)
    expect(get(overwriteMode)).toBe(false)
  })

  it('never throws on malformed / partial view objects', () => {
    expect(() => syncOverwriteFromView(null)).not.toThrow()
    expect(() => syncOverwriteFromView(undefined)).not.toThrow()
    expect(() => syncOverwriteFromView({})).not.toThrow()
    expect(syncOverwriteFromView({})).toBe(false)
  })
})
