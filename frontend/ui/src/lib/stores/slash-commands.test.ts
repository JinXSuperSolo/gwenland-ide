import { describe, it, expect } from 'vitest'
import {
  parseSlashQuery,
  filterCommands,
  exactCommand,
  parseHistoryCount,
  SLASH_COMMANDS,
} from './slash-commands'

describe('parseSlashQuery', () => {
  it('returns null for non-slash text', () => {
    expect(parseSlashQuery('hello')).toBeNull()
    expect(parseSlashQuery('')).toBeNull()
    // A slash mid-line is prose, not a command.
    expect(parseSlashQuery('what about a/b testing')).toBeNull()
  })

  it('parses a bare slash as an empty token', () => {
    expect(parseSlashQuery('/')).toEqual({ token: '', rest: '', hasArgs: false })
  })

  it('parses a command name without args', () => {
    expect(parseSlashQuery('/com')).toEqual({ token: 'com', rest: '', hasArgs: false })
    expect(parseSlashQuery('/CLEAR')).toEqual({ token: 'clear', rest: '', hasArgs: false })
  })

  it('splits args after the first space', () => {
    expect(parseSlashQuery('/get-history 10')).toEqual({
      token: 'get-history',
      rest: '10',
      hasArgs: true,
    })
  })

  it('only considers the first line', () => {
    expect(parseSlashQuery('/clear\nmore text')).toEqual({
      token: 'clear',
      rest: '',
      hasArgs: false,
    })
  })
})

describe('filterCommands', () => {
  it('returns all commands for an empty token', () => {
    expect(filterCommands('')).toHaveLength(SLASH_COMMANDS.length)
  })

  it('narrows by prefix', () => {
    const r = filterCommands('com')
    expect(r).toHaveLength(1)
    expect(r[0].id).toBe('compact')
  })

  it('ranks prefix matches before substring matches', () => {
    // "c" prefixes clear/compact; add-ctx-folder only contains it mid-word, so
    // the prefix matches must come first.
    const ids = filterCommands('c').map((c) => c.id)
    expect(ids.slice(0, 2)).toEqual(['clear', 'compact'])
    expect(ids.indexOf('add-ctx-folder')).toBeGreaterThan(1)
  })

  it('is case-insensitive', () => {
    expect(filterCommands('NEW').map((c) => c.id)).toContain('new')
  })
})

describe('exactCommand', () => {
  it('matches a fully-typed token', () => {
    expect(exactCommand('get-history')?.id).toBe('get-history')
    expect(exactCommand('GET-HISTORY')?.id).toBe('get-history')
  })
  it('returns null for a partial token', () => {
    expect(exactCommand('get')).toBeNull()
  })
})

describe('parseHistoryCount', () => {
  it('defaults to 5', () => {
    expect(parseHistoryCount('')).toBe(5)
    expect(parseHistoryCount('abc')).toBe(5)
    expect(parseHistoryCount('0')).toBe(5)
  })
  it('parses a positive count and clamps to 50', () => {
    expect(parseHistoryCount('10')).toBe(10)
    expect(parseHistoryCount('999')).toBe(50)
    expect(parseHistoryCount('3 extra')).toBe(3)
  })
})
