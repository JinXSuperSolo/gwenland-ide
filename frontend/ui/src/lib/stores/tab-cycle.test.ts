import { describe, expect, it } from 'vitest'
import { canSwitchTabs, initialSwitchIndex, stepSwitchIndex } from './tab-cycle'

/**
 * Behavioral tests for Ctrl+Tab / Ctrl+Shift+Tab editor-tab cycling (M-keynav §4).
 * The MRU quick switcher cycles live tab ids, wraps at the ends, and no-ops with
 * 0 or 1 open tab.
 */

describe('canSwitchTabs', () => {
  it('no-ops with zero or one tab, switches with two or more', () => {
    expect(canSwitchTabs(0)).toBe(false)
    expect(canSwitchTabs(1)).toBe(false)
    expect(canSwitchTabs(2)).toBe(true)
    expect(canSwitchTabs(5)).toBe(true)
  })
})

describe('initialSwitchIndex', () => {
  it('forward opens on the previous MRU tab (index 1)', () => {
    expect(initialSwitchIndex(3, false)).toBe(1)
  })

  it('reverse opens on the last tab', () => {
    expect(initialSwitchIndex(3, true)).toBe(2)
    expect(initialSwitchIndex(2, true)).toBe(1)
  })
})

describe('stepSwitchIndex', () => {
  it('forward steps right and wraps at the end', () => {
    // 3 tabs: 1 -> 2 -> 0 -> 1
    expect(stepSwitchIndex(1, 3, false)).toBe(2)
    expect(stepSwitchIndex(2, 3, false)).toBe(0)
    expect(stepSwitchIndex(0, 3, false)).toBe(1)
  })

  it('reverse steps left and wraps at the start', () => {
    // 3 tabs: 2 -> 1 -> 0 -> 2
    expect(stepSwitchIndex(2, 3, true)).toBe(1)
    expect(stepSwitchIndex(1, 3, true)).toBe(0)
    expect(stepSwitchIndex(0, 3, true)).toBe(2)
  })

  it('cycles a full loop back to the start (forward, 4 tabs)', () => {
    let i = initialSwitchIndex(4, false) // 1
    i = stepSwitchIndex(i, 4, false) // 2
    i = stepSwitchIndex(i, 4, false) // 3
    i = stepSwitchIndex(i, 4, false) // 0
    i = stepSwitchIndex(i, 4, false) // 1
    expect(i).toBe(1)
  })
})
