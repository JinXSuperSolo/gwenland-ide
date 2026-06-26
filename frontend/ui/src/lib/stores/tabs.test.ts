import { get } from 'svelte/store'
import { beforeEach, describe, expect, it } from 'vitest'
import { moveTabToGroup, tabs, type Tab, type TabsState } from './tabs'

const previewA: Tab = {
  id: 'a',
  kind: 'preview',
  name: 'a.png',
  source: { kind: 'static-file', path: '/ws/a.png' },
}

const previewB: Tab = {
  id: 'b',
  kind: 'preview',
  name: 'b.png',
  source: { kind: 'static-file', path: '/ws/b.png' },
}

function reset(next?: Partial<TabsState>) {
  tabs.set({
    tabs: [previewA, previewB],
    activeId: 'a',
    activeGroupId: 'left',
    orientation: 'horizontal',
    groups: [
      {
        id: 'left',
        tabs: [previewA],
        activeId: 'a',
        isLocked: false,
        isMaximized: false,
        size: 1,
      },
      {
        id: 'right',
        tabs: [previewB],
        activeId: 'b',
        isLocked: false,
        isMaximized: false,
        size: 1,
      },
    ],
    ...next,
  })
}

describe('tabs store group moves', () => {
  beforeEach(() => reset())

  it('moves an existing tab object between groups and activates the target group', () => {
    const original = get(tabs).groups[0].tabs[0]

    expect(moveTabToGroup('a', 'right', 0)).toBe(true)

    const state = get(tabs)
    expect(state.activeGroupId).toBe('right')
    expect(state.activeId).toBe('a')
    expect(state.groups[0].tabs).toEqual([])
    expect(state.groups[1].tabs.map((tab) => tab.id)).toEqual(['a', 'b'])
    expect(state.groups[1].tabs[0]).toBe(original)
  })

  it('reorders within the same group', () => {
    reset({
      tabs: [previewA, previewB],
      activeId: 'a',
      groups: [
        {
          id: 'left',
          tabs: [previewA, previewB],
          activeId: 'a',
          isLocked: false,
          isMaximized: false,
          size: 1,
        },
      ],
    })

    expect(moveTabToGroup('a', 'left', 2)).toBe(true)
    expect(get(tabs).groups[0].tabs.map((tab) => tab.id)).toEqual(['b', 'a'])
  })

  it('does not drop into a locked group', () => {
    reset({
      groups: [
        {
          id: 'left',
          tabs: [previewA],
          activeId: 'a',
          isLocked: false,
          isMaximized: false,
          size: 1,
        },
        {
          id: 'right',
          tabs: [previewB],
          activeId: 'b',
          isLocked: true,
          isMaximized: false,
          size: 1,
        },
      ],
    })

    expect(moveTabToGroup('a', 'right')).toBe(false)
    expect(get(tabs).groups[0].tabs.map((tab) => tab.id)).toEqual(['a'])
    expect(get(tabs).groups[1].tabs.map((tab) => tab.id)).toEqual(['b'])
  })
})
