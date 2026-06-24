import { describe, it, expect, beforeEach } from 'vitest'
import { ContextActionRegistry, isActionEnabled, registry } from './actionRegistry'
import type { ContextAction, ContextMenuContext } from './contextTypes'
import { registerContextActions } from './registerActions'
import { lsp } from '../stores/lsp'
import type { LspStatus } from '../tauri/commands'

/** Build a test action with sensible defaults. */
function makeAction(p: Partial<ContextAction> & Pick<ContextAction, 'id' | 'group' | 'order' | 'when'>): ContextAction {
  return { label: p.id, run: () => {}, ...p }
}

const ctx = (scope: ContextMenuContext['scope'], extra: Partial<ContextMenuContext> = {}): ContextMenuContext => ({
  scope,
  ...extra,
})

describe('ContextActionRegistry mechanics', () => {
  let r: ContextActionRegistry
  beforeEach(() => {
    r = new ContextActionRegistry()
  })

  it('filters actions by when(ctx)', () => {
    r.register(makeAction({ id: 'a', group: 'g', order: 10, when: (c) => c.scope === 'file_tree' }))
    r.register(makeAction({ id: 'b', group: 'g', order: 20, when: (c) => c.scope === 'editor' }))
    expect(r.getActions(ctx('file_tree')).map((a) => a.id)).toEqual(['a'])
    expect(r.getActions(ctx('editor')).map((a) => a.id)).toEqual(['b'])
  })

  it('orders groups by their minimum order, items by order within', () => {
    r.register(makeAction({ id: 'view1', group: 'view', order: 30, when: () => true }))
    r.register(makeAction({ id: 'create2', group: 'create', order: 20, when: () => true }))
    r.register(makeAction({ id: 'create1', group: 'create', order: 10, when: () => true }))
    const grouped = r.getGrouped(ctx('file_tree'))
    expect(grouped.map((g) => g.group)).toEqual(['create', 'view'])
    expect(grouped[0].actions.map((a) => a.id)).toEqual(['create1', 'create2'])
  })

  it('a throwing when() hides the action rather than crashing the menu', () => {
    r.register(
      makeAction({
        id: 'boom',
        group: 'g',
        order: 10,
        when: () => {
          throw new Error('boom')
        },
      }),
    )
    r.register(makeAction({ id: 'ok', group: 'g', order: 20, when: () => true }))
    expect(r.getActions(ctx('file_tree')).map((a) => a.id)).toEqual(['ok'])
  })

  it('register replaces an action with the same id', () => {
    r.register(makeAction({ id: 'a', group: 'g', order: 10, when: () => true, label: 'one' }))
    r.register(makeAction({ id: 'a', group: 'g', order: 10, when: () => true, label: 'two' }))
    expect(r.all()).toHaveLength(1)
    expect(r.getActions(ctx('file_tree'))[0].label).toBe('two')
  })

  it('isActionEnabled defaults to true and is throw-safe', () => {
    const noGate = makeAction({ id: 'a', group: 'g', order: 10, when: () => true })
    expect(isActionEnabled(noGate, ctx('file_tree'))).toBe(true)

    const gated = makeAction({ id: 'b', group: 'g', order: 10, when: () => true, enabled: (c) => !!c.path })
    expect(isActionEnabled(gated, ctx('file_tree'))).toBe(false)
    expect(isActionEnabled(gated, ctx('file_tree', { path: '/x' }))).toBe(true)

    const throws = makeAction({
      id: 'c',
      group: 'g',
      order: 10,
      when: () => true,
      enabled: () => {
        throw new Error('x')
      },
    })
    expect(isActionEnabled(throws, ctx('file_tree'))).toBe(false)
  })
})

describe('registered action sets (real registry)', () => {
  beforeEach(() => {
    // Idempotent: actions register by id, so repeated calls just re-set them.
    registerContextActions()
  })

  it('each scope sees only its own actions', () => {
    const fileIds = registry.getActions(ctx('file_tree', { path: '/ws/a.ts', workspaceRoot: '/ws' })).map((a) => a.id)
    expect(fileIds).toContain('file.newFile')
    expect(fileIds).toContain('file.delete')
    expect(fileIds).toContain('file.copyRelativePath')
    expect(fileIds.every((id) => id.startsWith('file.'))).toBe(true)

    const tabIds = registry.getActions(ctx('editor_tab', { tabId: 't1', path: '/ws/a.ts' })).map((a) => a.id)
    expect(tabIds).toContain('tab.close')
    expect(tabIds).toContain('tab.closeOthers')
    expect(tabIds.every((id) => id.startsWith('tab.'))).toBe(true)

    const terminalIds = registry.getActions(ctx('terminal', { terminalId: 'k' })).map((a) => a.id)
    expect(terminalIds).toContain('terminal.copy')
    expect(terminalIds.every((id) => id.startsWith('terminal.'))).toBe(true)
  })

  it('workspace_empty and git scopes are isolated', () => {
    const wsIds = registry.getActions(ctx('workspace_empty', { workspaceRoot: '/ws' })).map((a) => a.id)
    expect(wsIds).toContain('workspace.newFile')
    expect(wsIds.some((id) => id.startsWith('file.'))).toBe(false)

    const gitIds = registry.getActions(ctx('git', { path: '/ws/a.ts', gitStatus: 'M' })).map((a) => a.id)
    expect(gitIds).toContain('git.discard')
    expect(gitIds.every((id) => id.startsWith('git.'))).toBe(true)
  })

  it('"Open to Side" hides on folders, shows on files', () => {
    const onFolder = registry.getActions(ctx('file_tree', { path: '/ws/d', isDirectory: true, workspaceRoot: '/ws' }))
    expect(onFolder.some((a) => a.id === 'file.openToSide')).toBe(false)
    const onFile = registry.getActions(ctx('file_tree', { path: '/ws/a.ts', isDirectory: false, workspaceRoot: '/ws' }))
    expect(onFile.some((a) => a.id === 'file.openToSide')).toBe(true)
  })

  it('LSP editor actions disable without a connected server and enable with one', () => {
    const path = '/ws/a.rs'
    const rename = registry.getActions(ctx('editor', { path })).find((a) => a.id === 'editor.renameSymbol')
    expect(rename).toBeDefined()

    lsp.set({ status: {}, diagnostics: {} })
    expect(isActionEnabled(rename!, ctx('editor', { path }))).toBe(false)

    const connected: LspStatus = { state: 'connected', language: 'rust', server_name: null }
    lsp.set({ status: { [path]: connected }, diagnostics: {} })
    expect(isActionEnabled(rename!, ctx('editor', { path }))).toBe(true)
  })

  it('global + input fallback scopes are populated and isolated', () => {
    const globalIds = registry.getActions(ctx('global')).map((a) => a.id)
    expect(globalIds).toContain('global.commandPalette')
    expect(globalIds).toContain('global.settings')
    expect(globalIds.every((id) => id.startsWith('global.'))).toBe(true)

    const inputIds = registry.getActions(ctx('input')).map((a) => a.id)
    expect(inputIds).toEqual(['input.cut', 'input.copy', 'input.paste', 'input.selectAll'])

    // The fallback scopes never bleed into a real surface scope.
    const fileIds = registry.getActions(ctx('file_tree', { path: '/ws/a', workspaceRoot: '/ws' })).map((a) => a.id)
    expect(fileIds.some((id) => id.startsWith('global.') || id.startsWith('input.'))).toBe(false)
  })

  it('editor Copy/Cut gate on a selection; Paste does not', () => {
    const path = '/ws/a.rs'
    const editorActs = registry.getActions(ctx('editor', { path }))
    const copy = editorActs.find((a) => a.id === 'editor.copy')!
    const paste = editorActs.find((a) => a.id === 'editor.paste')!
    expect(isActionEnabled(copy, ctx('editor', { path }))).toBe(false)
    expect(isActionEnabled(copy, ctx('editor', { path, selectionText: 'hi' }))).toBe(true)
    expect(isActionEnabled(paste, ctx('editor', { path }))).toBe(true)
  })
})
