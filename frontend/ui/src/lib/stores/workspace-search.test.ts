import { describe, expect, it } from 'vitest'
import type { WorkspaceSearchResult } from '../tauri/commands'
import { groupWorkspaceSearchResults } from './workspace-search'

function result(path: string, line: number): WorkspaceSearchResult {
  const relative = path.replace('/ws/', '')
  return {
    path,
    relative_path: relative,
    line_number: line,
    line: `line ${line}`,
  }
}

describe('groupWorkspaceSearchResults', () => {
  it('groups streamed results by first-seen file path', () => {
    const groups = groupWorkspaceSearchResults([
      result('/ws/src/a.ts', 1),
      result('/ws/src/b.ts', 2),
      result('/ws/src/a.ts', 3),
    ])

    expect(groups.map((group) => group.relativePath)).toEqual(['src/a.ts', 'src/b.ts'])
    expect(groups[0].results.map((item) => item.line_number)).toEqual([1, 3])
    expect(groups[1].results.map((item) => item.line_number)).toEqual([2])
  })

  it('returns an empty list for no results', () => {
    expect(groupWorkspaceSearchResults([])).toEqual([])
  })
})
