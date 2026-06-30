import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

type StorageListener = (event: StorageEvent) => void

function installWindowStub(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial))
  let storageListener: StorageListener | null = null

  const localStorage = {
    getItem: vi.fn((key: string) => values.get(key) ?? null),
    setItem: vi.fn((key: string, value: string) => {
      values.set(key, value)
    }),
    removeItem: vi.fn((key: string) => {
      values.delete(key)
    }),
    clear: vi.fn(() => {
      values.clear()
    }),
  }

  vi.stubGlobal('window', {
    localStorage,
    addEventListener: vi.fn((type: string, listener: StorageListener) => {
      if (type === 'storage') storageListener = listener
    }),
  })

  return {
    localStorage,
    emitStorage(newValue: string | null, key = 'gwenland.sidebarTab') {
      storageListener?.({ key, newValue } as StorageEvent)
    },
  }
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.resetModules()
})

describe('sidebar tab persistence', () => {
  it('restores the stored tab and persists tab changes', async () => {
    const stub = installWindowStub({ 'gwenland.sidebarTab': 'agent' })
    const sidebar = await import('./sidebar')

    sidebar.initSidebarTabPersistence()

    expect(get(sidebar.sidebarTab)).toBe('agent')
    sidebar.showFilesTab()
    expect(get(sidebar.sidebarTab)).toBe('files')
    expect(stub.localStorage.setItem).toHaveBeenLastCalledWith('gwenland.sidebarTab', 'files')
  })

  it('ignores invalid stored values and applies valid storage events', async () => {
    const stub = installWindowStub({ 'gwenland.sidebarTab': 'search' })
    const sidebar = await import('./sidebar')

    sidebar.initSidebarTabPersistence()

    expect(get(sidebar.sidebarTab)).toBe('files')
    stub.emitStorage('agent')
    expect(get(sidebar.sidebarTab)).toBe('agent')

    stub.emitStorage('git')
    expect(get(sidebar.sidebarTab)).toBe('agent')
  })
})
