import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Terminal } from '@xterm/xterm'
import { TerminalScheduler, mergeChunks } from './terminal-scheduler'

/**
 * Tests for the terminal output frame-limiter (M19 Wave 4). rAF is stubbed so a
 * "frame" fires only when we call `flushFrame()`, letting us assert coalescing
 * and pause/resume deterministically.
 */

let rafCallbacks: FrameRequestCallback[] = []

function flushFrame() {
  const cbs = rafCallbacks
  rafCallbacks = []
  for (const cb of cbs) cb(0)
}

/** Minimal Terminal stand-in capturing what gets written. */
function fakeTerm(): { term: Terminal; writes: Uint8Array[] } {
  const writes: Uint8Array[] = []
  const term = {
    write: (data: Uint8Array) => {
      writes.push(data)
    },
  } as unknown as Terminal
  return { term, writes }
}

const bytes = (s: string) => new TextEncoder().encode(s)

beforeEach(() => {
  rafCallbacks = []
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
    rafCallbacks.push(cb)
    return rafCallbacks.length
  })
  vi.stubGlobal('cancelAnimationFrame', () => {
    rafCallbacks = []
  })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('mergeChunks', () => {
  it('returns the single chunk unchanged', () => {
    const a = bytes('hi')
    expect(mergeChunks([a])).toBe(a)
  })

  it('concatenates multiple chunks in order', () => {
    const merged = mergeChunks([bytes('ab'), bytes('cd'), bytes('e')])
    expect(new TextDecoder().decode(merged)).toBe('abcde')
  })
})

describe('TerminalScheduler', () => {
  it('coalesces a burst into a single write per frame', () => {
    const { term, writes } = fakeTerm()
    const s = new TerminalScheduler(term)
    for (let i = 0; i < 100; i++) s.write(bytes('x'))
    expect(writes.length).toBe(0) // nothing until the frame fires
    flushFrame()
    expect(writes.length).toBe(1) // 100 chunks → one write
    expect(writes[0].length).toBe(100)
  })

  it('does not write while paused, buffers, then flushes on resume', () => {
    const { term, writes } = fakeTerm()
    const s = new TerminalScheduler(term)
    s.pause()
    s.write(bytes('a'))
    s.write(bytes('b'))
    flushFrame()
    expect(writes.length).toBe(0) // paused: no frame scheduled
    s.resume()
    expect(writes.length).toBe(1)
    expect(new TextDecoder().decode(writes[0])).toBe('ab')
  })

  it('drops nothing on resume when buffer is empty', () => {
    const { term, writes } = fakeTerm()
    const s = new TerminalScheduler(term)
    s.pause()
    s.resume()
    expect(writes.length).toBe(0)
  })

  it('stops writing after dispose', () => {
    const { term, writes } = fakeTerm()
    const s = new TerminalScheduler(term)
    s.write(bytes('a'))
    s.dispose()
    flushFrame()
    expect(writes.length).toBe(0)
  })
})
