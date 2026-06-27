import { describe, expect, it } from 'vitest'
import type { FsPatch } from '../tauri/commands'
import { treeRefreshPathsFromFsPatches } from './fs-watch'

function patch(partial: Partial<FsPatch>): FsPatch {
  return {
    dir: '/ws',
    added: [],
    removed: [],
    modified: [],
    ...partial,
  }
}

describe('treeRefreshPathsFromFsPatches', () => {
  it('skips content-only file modifications', () => {
    expect(
      treeRefreshPathsFromFsPatches([
        patch({ dir: '/ws/src', modified: ['/ws/src/main.ts'] }),
      ]),
    ).toEqual([])
  })

  it('keeps structural directory patches and modified dirs', () => {
    expect(
      treeRefreshPathsFromFsPatches([
        patch({ dir: '/ws/src', added: ['/ws/src/new.ts'] }),
        patch({ dir: '/ws', modified_dirs: ['/ws/src'] }),
      ]),
    ).toEqual(['/ws/src', '/ws/src'])
  })
})
