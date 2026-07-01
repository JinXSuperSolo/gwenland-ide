import { describe, expect, it } from 'vitest'
import { mimeToExt, attachmentIconSvg, formatFileSize, truncateFileName } from './file-attachment'

describe('mimeToExt', () => {
  it('maps exact known MIME types', () => {
    expect(mimeToExt('application/pdf')).toBe('pdf')
    expect(mimeToExt('application/json')).toBe('json')
    expect(mimeToExt('image/svg+xml')).toBe('svg')
    expect(mimeToExt('text/markdown')).toBe('md')
  })

  it('falls back to the subtype for broad categories', () => {
    expect(mimeToExt('image/png')).toBe('png')
    expect(mimeToExt('image/webp')).toBe('webp')
    expect(mimeToExt('video/mp4')).toBe('mp4')
  })

  it('normalizes audio/mpeg to mp3 and text/* to txt', () => {
    expect(mimeToExt('audio/mpeg')).toBe('mp3')
    expect(mimeToExt('text/plain')).toBe('txt')
  })

  it('returns empty string for unknown/opaque types', () => {
    expect(mimeToExt('application/octet-stream')).toBe('')
  })
})

describe('attachmentIconSvg', () => {
  it('returns a non-empty SVG string for a filename', () => {
    const svg = attachmentIconSvg({ name: 'main.rs' })
    expect(svg).toContain('<svg')
  })

  it('returns an SVG for a MIME-only image attachment', () => {
    const svg = attachmentIconSvg({ mime: 'image/png' })
    expect(svg).toContain('<svg')
  })

  it('falls back to a generic document icon when nothing is known', () => {
    const svg = attachmentIconSvg({})
    expect(svg).toContain('<svg')
  })
})

describe('formatFileSize', () => {
  it('shows bytes under 1 KB', () => {
    expect(formatFileSize(0)).toBe('0 B')
    expect(formatFileSize(512)).toBe('512 B')
    expect(formatFileSize(1023)).toBe('1023 B')
  })

  it('shows one decimal below 10 units, whole numbers above', () => {
    expect(formatFileSize(1024)).toBe('1 KB')
    expect(formatFileSize(1536)).toBe('1.5 KB')
    expect(formatFileSize(15 * 1024)).toBe('15 KB')
    expect(formatFileSize(1.4 * 1024 * 1024)).toBe('1.4 MB')
    expect(formatFileSize(24 * 1024 * 1024)).toBe('24 MB')
  })

  it('scales into GB', () => {
    expect(formatFileSize(3 * 1024 * 1024 * 1024)).toBe('3 GB')
  })

  it('returns empty for null/negative/NaN', () => {
    expect(formatFileSize(null)).toBe('')
    expect(formatFileSize(undefined)).toBe('')
    expect(formatFileSize(-5)).toBe('')
    expect(formatFileSize(NaN)).toBe('')
  })
})

describe('truncateFileName', () => {
  it('leaves short names unchanged', () => {
    expect(truncateFileName('main.rs')).toBe('main.rs')
    expect(truncateFileName('a'.repeat(28))).toBe('a'.repeat(28))
  })

  it('middle-truncates keeping the extension visible', () => {
    const out = truncateFileName('really-long-component-name.svelte')
    expect(out.length).toBeLessThanOrEqual(28)
    expect(out.endsWith('.svelte')).toBe(true)
    expect(out).toContain('…')
    // Keeps a bit of the head and the tail of the base name.
    expect(out.startsWith('really')).toBe(true)
  })

  it('head-truncates dotfiles / extensionless names', () => {
    const out = truncateFileName('a-really-long-name-with-no-extension', 20)
    expect(out.length).toBeLessThanOrEqual(20)
    expect(out.endsWith('…')).toBe(true)
  })

  it('survives a name whose extension nearly fills the budget', () => {
    const out = truncateFileName('x.superlongextension', 12)
    expect(out.length).toBeLessThanOrEqual(12)
    expect(out).toContain('…')
  })
})
