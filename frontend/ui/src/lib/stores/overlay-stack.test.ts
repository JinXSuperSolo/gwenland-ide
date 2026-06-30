import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  closeTopmost,
  markOverlayClosed,
  markOverlayOpen,
  openOverlayCount,
  registerOverlay,
  resetOverlayStack,
  topmostOverlayId,
} from './overlay-stack'

/**
 * Behavioral tests for the centralized Escape overlay stack (M-keynav §2). The
 * stack must close exactly ONE overlay per Escape — the most-recently-opened —
 * and be a no-op when nothing is open.
 */

afterEach(() => {
  resetOverlayStack()
  vi.restoreAllMocks()
})

describe('overlay stack', () => {
  it('closeTopmost is a no-op (returns false) when nothing is open', () => {
    const close = vi.fn()
    registerOverlay({ id: 'palette', close })
    expect(closeTopmost()).toBe(false)
    expect(close).not.toHaveBeenCalled()
  })

  it('closes exactly the topmost (most-recently-opened) overlay per press', () => {
    const closePalette = vi.fn()
    const closeAbout = vi.fn()
    registerOverlay({ id: 'palette', close: closePalette })
    registerOverlay({ id: 'about', close: closeAbout })

    // Palette opens first, then About stacks on top.
    markOverlayOpen('palette')
    markOverlayOpen('about')
    expect(topmostOverlayId()).toBe('about')
    expect(openOverlayCount()).toBe(2)

    // First Escape closes only About.
    expect(closeTopmost()).toBe(true)
    expect(closeAbout).toHaveBeenCalledTimes(1)
    expect(closePalette).not.toHaveBeenCalled()
    expect(openOverlayCount()).toBe(1)
    expect(topmostOverlayId()).toBe('palette')

    // Second Escape closes Palette.
    expect(closeTopmost()).toBe(true)
    expect(closePalette).toHaveBeenCalledTimes(1)
    expect(openOverlayCount()).toBe(0)

    // Third Escape is a no-op.
    expect(closeTopmost()).toBe(false)
  })

  it('treats re-opening an overlay as moving it back to the top', () => {
    const a = vi.fn()
    const b = vi.fn()
    registerOverlay({ id: 'a', close: a })
    registerOverlay({ id: 'b', close: b })
    markOverlayOpen('a')
    markOverlayOpen('b')
    // Re-open 'a' — it should now be topmost again.
    markOverlayOpen('a')
    expect(topmostOverlayId()).toBe('a')
    closeTopmost()
    expect(a).toHaveBeenCalledTimes(1)
    expect(b).not.toHaveBeenCalled()
  })

  it('drops an overlay from the stack when it closes itself', () => {
    const a = vi.fn()
    registerOverlay({ id: 'a', close: a })
    markOverlayOpen('a')
    markOverlayClosed('a')
    expect(closeTopmost()).toBe(false)
    expect(a).not.toHaveBeenCalled()
  })

  it('unregister removes the handler and any open membership', () => {
    const a = vi.fn()
    const dispose = registerOverlay({ id: 'a', close: a })
    markOverlayOpen('a')
    dispose()
    expect(closeTopmost()).toBe(false)
    expect(a).not.toHaveBeenCalled()
  })
})
