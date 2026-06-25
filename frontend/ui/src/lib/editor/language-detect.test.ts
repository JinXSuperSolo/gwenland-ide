import { describe, expect, it } from 'vitest'
import { languageIdForFilename } from './language-detect'

describe('languageIdForFilename', () => {
  it('maps common editor extensions to syntax modes', () => {
    expect(languageIdForFilename('src/App.svelte')).toBe('html')
    expect(languageIdForFilename('src/main.rs')).toBe('rust')
    expect(languageIdForFilename('lib/index.ts')).toBe('javascript')
    expect(languageIdForFilename('script.py')).toBe('python')
    expect(languageIdForFilename('package.json')).toBe('json')
    expect(languageIdForFilename('README.md')).toBe('markdown')
    expect(languageIdForFilename('styles.scss')).toBe('css')
  })

  it('returns null for unsupported or extensionless files', () => {
    expect(languageIdForFilename('LICENSE')).toBeNull()
    expect(languageIdForFilename('archive.bin')).toBeNull()
  })
})
