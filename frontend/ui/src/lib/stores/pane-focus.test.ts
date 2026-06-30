import { describe, expect, it } from 'vitest'
import {
  availablePanes,
  nextPane,
  shouldCyclePanes,
  type PaneAvailability,
} from './pane-focus'

/**
 * Behavioral tests for Tab pane-cycling (M-keynav §3). Order is Sidebar → Editor
 * → Terminal with wrap; unavailable panes (collapsed terminal / no workspace) are
 * skipped; and pane-cycling is suppressed while focus is in an editable surface
 * so in-editor Tab (indentation) and form Tab keep working.
 */

const ALL: PaneAvailability = { sidebar: true, editor: true, terminal: true }

/** Minimal Element-like stub (vitest runs in node; no real DOM). */
function el(opts: {
  tag?: string
  contentEditable?: boolean
  inCm?: boolean
}): Element {
  return {
    tagName: opts.tag ?? 'DIV',
    isContentEditable: opts.contentEditable ?? false,
    closest: (sel: string) => (opts.inCm && sel === '.cm-editor' ? ({} as Element) : null),
  } as unknown as Element
}

describe('availablePanes', () => {
  it('keeps the canonical order, filtering unavailable panes', () => {
    expect(availablePanes(ALL)).toEqual(['sidebar', 'editor', 'terminal'])
    expect(availablePanes({ sidebar: false, editor: true, terminal: false })).toEqual(['editor'])
    expect(availablePanes({ sidebar: true, editor: true, terminal: false })).toEqual([
      'sidebar',
      'editor',
    ])
  })
})

describe('nextPane (forward = Tab)', () => {
  it('cycles Sidebar → Editor → Terminal → Sidebar', () => {
    expect(nextPane('sidebar', 1, ALL)).toBe('editor')
    expect(nextPane('editor', 1, ALL)).toBe('terminal')
    expect(nextPane('terminal', 1, ALL)).toBe('sidebar')
  })

  it('lands on the first pane when nothing is focused yet', () => {
    expect(nextPane(null, 1, ALL)).toBe('sidebar')
  })

  it('skips the terminal when it is unavailable', () => {
    const noTerm: PaneAvailability = { sidebar: true, editor: true, terminal: false }
    expect(nextPane('editor', 1, noTerm)).toBe('sidebar') // wraps past the missing terminal
    expect(nextPane('sidebar', 1, noTerm)).toBe('editor')
  })
})

describe('nextPane (reverse = Shift+Tab)', () => {
  it('cycles in reverse Sidebar → Terminal → Editor → Sidebar', () => {
    expect(nextPane('sidebar', -1, ALL)).toBe('terminal')
    expect(nextPane('terminal', -1, ALL)).toBe('editor')
    expect(nextPane('editor', -1, ALL)).toBe('sidebar')
  })

  it('lands on the last pane when nothing is focused yet', () => {
    expect(nextPane(null, -1, ALL)).toBe('terminal')
  })
})

describe('nextPane edge cases', () => {
  it('returns null when no pane is available', () => {
    expect(nextPane('editor', 1, { sidebar: false, editor: false, terminal: false })).toBeNull()
  })

  it('falls back to first/last when the current pane is unavailable', () => {
    const noTerm: PaneAvailability = { sidebar: true, editor: true, terminal: false }
    // 'terminal' isn't in the available list -> treated as unknown.
    expect(nextPane('terminal', 1, noTerm)).toBe('sidebar')
    expect(nextPane('terminal', -1, noTerm)).toBe('editor')
  })
})

describe('shouldCyclePanes (in-editor Tab guard)', () => {
  it('allows pane-cycling from a non-editable pane container', () => {
    expect(shouldCyclePanes(el({ tag: 'DIV' }))).toBe(true)
    expect(shouldCyclePanes(null)).toBe(true)
  })

  it('suppresses pane-cycling inside the CodeMirror editor (so Tab indents)', () => {
    expect(shouldCyclePanes(el({ tag: 'DIV', contentEditable: true, inCm: true }))).toBe(false)
  })

  it('suppresses pane-cycling inside text inputs and textareas', () => {
    expect(shouldCyclePanes(el({ tag: 'INPUT' }))).toBe(false)
    expect(shouldCyclePanes(el({ tag: 'TEXTAREA' }))).toBe(false)
  })

  it('suppresses pane-cycling inside any contenteditable element', () => {
    expect(shouldCyclePanes(el({ tag: 'DIV', contentEditable: true }))).toBe(false)
  })
})
