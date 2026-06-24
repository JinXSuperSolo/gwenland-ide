import { describe, it, expect } from 'vitest'
import {
  parseMentionQuery,
  parseLineRange,
  fuzzyScore,
  fuzzySearch,
  stripHtml,
  iconForPath,
  type WorkspaceEntry,
} from './mention-providers'

describe('parseMentionQuery', () => {
  it('returns null when there is no @ before the caret', () => {
    expect(parseMentionQuery('hello world', 11)).toBeNull()
    expect(parseMentionQuery('', 0)).toBeNull()
  })

  it('detects an @ at the start of the text', () => {
    expect(parseMentionQuery('@git', 4)).toEqual({ at: 0, query: 'git' })
  })

  it('detects an @ after whitespace', () => {
    expect(parseMentionQuery('see @main', 9)).toEqual({ at: 4, query: 'main' })
  })

  it('rejects an @ that is not at a token boundary (e.g. email)', () => {
    expect(parseMentionQuery('a@b', 3)).toBeNull()
  })

  it('ends a normal mention at a space', () => {
    expect(parseMentionQuery('@git done', 9)).toBeNull()
  })

  it('allows one trailing arg for @web', () => {
    expect(parseMentionQuery('@web https://x.com', 18)).toEqual({
      at: 0,
      query: 'web https://x.com',
    })
  })

  it('reads only up to the caret', () => {
    expect(parseMentionQuery('@diagnostics', 5)).toEqual({ at: 0, query: 'diag' })
  })

  it('stops at a newline', () => {
    expect(parseMentionQuery('@git\nmore', 9)).toBeNull()
  })
})

describe('parseLineRange', () => {
  it('returns the path unchanged with no range', () => {
    expect(parseLineRange('src/main.rs')).toEqual({ path: 'src/main.rs' })
  })

  it('parses a start-end range', () => {
    expect(parseLineRange('src/main.rs:10-50')).toEqual({
      path: 'src/main.rs',
      lStart: 10,
      lEnd: 50,
    })
  })

  it('parses a single-line range', () => {
    expect(parseLineRange('a.ts:42')).toEqual({ path: 'a.ts', lStart: 42, lEnd: 42 })
  })

  it('normalizes a reversed range', () => {
    expect(parseLineRange('a.ts:50-10')).toEqual({ path: 'a.ts', lStart: 10, lEnd: 50 })
  })

  it('does not treat a Windows drive letter as a range', () => {
    // `C:\path` — the colon is followed by a backslash, not digits.
    expect(parseLineRange('C:\\path\\file.rs')).toEqual({ path: 'C:\\path\\file.rs' })
  })
})

describe('fuzzyScore', () => {
  it('ranks exact > prefix > boundary > substring > subsequence', () => {
    const exact = fuzzyScore('main', 'main')
    const prefix = fuzzyScore('mai', 'main.rs')
    const boundary = fuzzyScore('rs', 'main.rs')
    const substr = fuzzyScore('ain', 'main.rs')
    const subseq = fuzzyScore('mrs', 'main.rs')
    expect(exact).toBeGreaterThan(prefix)
    expect(prefix).toBeGreaterThan(boundary)
    expect(boundary).toBeGreaterThan(substr)
    expect(substr).toBeGreaterThan(subseq)
    expect(subseq).toBeGreaterThan(0)
  })

  it('returns 0 for no match', () => {
    expect(fuzzyScore('xyz', 'main.rs')).toBe(0)
  })

  it('matches everything for an empty query', () => {
    expect(fuzzyScore('', 'anything')).toBeGreaterThan(0)
  })
})

describe('fuzzySearch', () => {
  const entries: WorkspaceEntry[] = [
    { path: '/w/src/main.rs', rel: 'src/main.rs', isDir: false },
    { path: '/w/src/lib.rs', rel: 'src/lib.rs', isDir: false },
    { path: '/w/src/components', rel: 'src/components', isDir: true },
    { path: '/w/README.md', rel: 'README.md', isDir: false },
  ]

  it('finds files by basename and ranks closest first', () => {
    const r = fuzzySearch('main', entries)
    expect(r[0].path).toBe('/w/src/main.rs')
    expect(r[0].type).toBe('file')
  })

  it('marks folders and appends a trailing slash to the insert', () => {
    const r = fuzzySearch('components', entries)
    expect(r[0].type).toBe('folder')
    expect(r[0].insert).toBe('src/components/')
  })

  it('returns nothing for a non-match', () => {
    expect(fuzzySearch('zzzzz', entries)).toEqual([])
  })

  it('uses the basename as the label', () => {
    const r = fuzzySearch('readme', entries)
    expect(r[0].label).toBe('README.md')
  })
})

describe('stripHtml', () => {
  it('removes tags and keeps text', () => {
    expect(stripHtml('<p>Hello <b>world</b></p>')).toBe('Hello world')
  })

  it('drops script and style blocks entirely', () => {
    const html = '<style>.a{color:red}</style><p>Keep</p><script>alert(1)<\/script>'
    expect(stripHtml(html)).toBe('Keep')
  })

  it('decodes common named entities', () => {
    expect(stripHtml('a &amp; b &lt;c&gt; &nbsp;d')).toBe('a & b <c> d')
  })

  it('decodes numeric entities (decimal and hex)', () => {
    expect(stripHtml('&#65;&#x42;')).toBe('AB')
  })

  it('turns block closes and <br> into newlines', () => {
    expect(stripHtml('<p>one</p><p>two</p>')).toBe('one\ntwo')
    expect(stripHtml('a<br>b')).toBe('a\nb')
  })

  it('collapses runs of whitespace and caps blank lines', () => {
    expect(stripHtml('<p>x</p>\n\n\n\n<p>y</p>')).toBe('x\n\ny')
    expect(stripHtml('a       b')).toBe('a b')
  })

  it('leaves an unknown entity untouched', () => {
    expect(stripHtml('&unknownentity;')).toBe('&unknownentity;')
  })
})

describe('iconForPath', () => {
  it('uses folder for directories', () => {
    expect(iconForPath('src', true)).toBe('folder')
  })
  it('uses code for source files', () => {
    expect(iconForPath('main.rs', false)).toBe('code')
    expect(iconForPath('a.tsx', false)).toBe('code')
  })
  it('uses page for non-code files', () => {
    expect(iconForPath('README.md', false)).toBe('page')
    expect(iconForPath('data.json', false)).toBe('page')
  })
})
