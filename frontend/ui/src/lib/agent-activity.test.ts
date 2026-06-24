import { describe, expect, it } from 'vitest'
import type { ToolLogEntry } from './stores/agentic'
import {
  activityFromToolEntry,
  currentActivityLabel,
  kindForTool,
  pendingActivity,
  runState,
} from './agent-activity'

const entry = (over: Partial<ToolLogEntry>): ToolLogEntry => ({
  id: 't1',
  tool: 'read_file',
  args: '{}',
  ok: null,
  content: null,
  error: null,
  ...over,
})

describe('agent-activity model', () => {
  it('maps tools to kinds', () => {
    expect(kindForTool('read_file')).toBe('read_file')
    expect(kindForTool('grep_search')).toBe('search')
    expect(kindForTool('edit_file')).toBe('edit_file')
    expect(kindForTool('run_terminal_cmd')).toBe('run_command')
    expect(kindForTool('ask_user')).toBe('approval')
  })

  it('derives a running read activity with path', () => {
    const a = activityFromToolEntry(entry({ tool: 'read_file', args: '{"path":"src/main.rs"}' }))
    expect(a.kind).toBe('read_file')
    expect(a.label).toBe('Reading src/main.rs')
    expect(a.status).toBe('running')
  })

  it('summarizes a finished search result and a failure', () => {
    const ok = activityFromToolEntry(
      entry({ id: 's', tool: 'file_search', args: '{"query":"Layout"}', ok: true, content: 'app/Layout.tsx\nother' })
    )
    expect(ok.status).toBe('ok')
    expect(ok.detail).toBe('→ app/Layout.tsx')

    const bad = activityFromToolEntry(entry({ ok: false, error: 'not found' }))
    expect(bad.status).toBe('failed')
    expect(bad.detail).toBe('not found')
  })

  it('labels the current activity from pending / running / busy', () => {
    const toolLog = [entry({ id: 'e', tool: 'run_terminal_cmd', args: '{"command":"npm test"}' })]
    expect(currentActivityLabel({ toolLog, pendingTool: null, pendingAsk: null, busy: true })).toBe(
      'Running npm test'
    )
    const pend = pendingActivity({
      toolLog: [entry({ id: 'p', tool: 'edit_file', args: '{"path":"src/x.ts"}' })],
      pendingTool: { id: 'p', tool: 'edit_file', side: 'mutating', risk: null },
      pendingAsk: null,
      busy: false,
    })
    expect(pend?.label).toContain('Waiting for approval')
    expect(pend?.label).toContain('src/x.ts')
  })

  it('computes run state', () => {
    expect(runState({ active: false, busy: false, stopped: null, hasError: false, cancelled: false })).toBe('idle')
    expect(runState({ active: true, busy: true, stopped: null, hasError: false, cancelled: false })).toBe('active')
    expect(runState({ active: false, busy: false, stopped: 'final', hasError: false, cancelled: false })).toBe('completed')
    expect(runState({ active: false, busy: false, stopped: null, hasError: true, cancelled: false })).toBe('failed')
    expect(runState({ active: false, busy: false, stopped: 'final', hasError: false, cancelled: true })).toBe('cancelled')
  })
})
