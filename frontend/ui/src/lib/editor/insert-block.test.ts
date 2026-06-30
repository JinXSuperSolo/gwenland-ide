import { get } from 'svelte/store'
import { afterEach, describe, expect, it, vi } from 'vitest'

// codemirror-setup pulls in the tauri command bridge; stub it so the module
// imports cleanly in the node test environment (no real Tauri runtime).
vi.mock('../tauri/commands', () => ({
  lspCompletion: vi.fn(),
  lspHover: vi.fn(),
  openBrowser: vi.fn(),
}))

import { insertBlockBindings } from './codemirror-setup'
import { overwriteMode } from './overwrite-mode'

/**
 * Behavioral test for the Insert-key hard block (M-keynav §5). The binding must
 * swallow the Insert key (run -> true, so CodeMirror treats it handled and the
 * default is prevented) and never flip the editor into overwrite mode.
 */

afterEach(() => overwriteMode.set(false))

describe('Insert-key block binding', () => {
  it('binds the Insert key with preventDefault', () => {
    const insert = insertBlockBindings.find((b) => b.key === 'Insert')
    expect(insert).toBeDefined()
    expect(insert?.preventDefault).toBe(true)
  })

  it('consumes the Insert key (run returns true) so nothing else acts on it', () => {
    const insert = insertBlockBindings.find((b) => b.key === 'Insert')!
    // A minimal view stub with no overwrite state.
    const handled = insert.run({ inputState: { overwrite: false } } as never)
    expect(handled).toBe(true)
  })

  it('keeps overwrite mode off after Insert is pressed', () => {
    const insert = insertBlockBindings.find((b) => b.key === 'Insert')!
    insert.run({ inputState: { overwrite: false } } as never)
    expect(get(overwriteMode)).toBe(false)
  })
})
