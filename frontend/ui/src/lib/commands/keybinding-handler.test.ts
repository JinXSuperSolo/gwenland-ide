import { afterEach, describe, expect, it, vi } from 'vitest'

type TestKeyEvent = KeyboardEvent & {
  preventDefault: ReturnType<typeof vi.fn>
  stopPropagation: ReturnType<typeof vi.fn>
}

function keyEvent(
  code: string,
  key: string,
  modifiers: Partial<Pick<KeyboardEvent, 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>> = {},
): TestKeyEvent {
  return {
    code,
    key,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
    ...modifiers,
  } as TestKeyEvent
}

async function flushCommands(): Promise<void> {
  await Promise.resolve()
}

afterEach(() => {
  vi.useRealTimers()
  vi.resetModules()
})

describe('global keybinding handler', () => {
  it('dispatches a registered single-stroke keybinding', async () => {
    const { registerCommand } = await import('./registry')
    const { handleGlobalKeydown } = await import('./keybinding-handler')
    const handler = vi.fn()
    registerCommand({
      id: 'test.single',
      title: 'Single',
      category: 'Test',
      defaultKeybinding: 'Ctrl+Alt+T',
      handler,
    })
    const event = keyEvent('KeyT', 't', { ctrlKey: true, altKey: true })

    expect(handleGlobalKeydown(event)).toBe(true)
    await flushCommands()

    expect(event.preventDefault).toHaveBeenCalledTimes(1)
    expect(event.stopPropagation).toHaveBeenCalledTimes(1)
    expect(handler).toHaveBeenCalledTimes(1)
  })

  it('dispatches a registered chord only after the second stroke', async () => {
    vi.useFakeTimers()
    const { registerCommand } = await import('./registry')
    const { handleGlobalKeydown } = await import('./keybinding-handler')
    const handler = vi.fn()
    registerCommand({
      id: 'test.chord',
      title: 'Chord',
      category: 'Test',
      defaultKeybinding: 'Ctrl+K Ctrl+T',
      handler,
    })

    const first = keyEvent('KeyK', 'k', { ctrlKey: true })
    expect(handleGlobalKeydown(first)).toBe(true)
    await flushCommands()
    expect(handler).not.toHaveBeenCalled()

    const second = keyEvent('KeyT', 't', { ctrlKey: true })
    expect(handleGlobalKeydown(second)).toBe(true)
    await flushCommands()

    expect(first.preventDefault).toHaveBeenCalledTimes(1)
    expect(second.preventDefault).toHaveBeenCalledTimes(1)
    expect(handler).toHaveBeenCalledTimes(1)
  })

  it('leaves unknown shortcuts untouched', async () => {
    const { handleGlobalKeydown } = await import('./keybinding-handler')
    const event = keyEvent('KeyQ', 'q', { ctrlKey: true, altKey: true })

    expect(handleGlobalKeydown(event)).toBe(false)

    expect(event.preventDefault).not.toHaveBeenCalled()
    expect(event.stopPropagation).not.toHaveBeenCalled()
  })
})
