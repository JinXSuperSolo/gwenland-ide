import { get } from 'svelte/store'
import { beforeEach, describe, expect, it } from 'vitest'
import type { AgentPlan, AgentSession, ChangeSet, ContextPreview } from '../tauri/commands'
import {
  agentic,
  addApproval,
  hasApprovedChange,
  includedContextIds,
  isPlanApproved,
  latestChangeSet,
  resetAgent,
  setContextPreview,
  setSession,
  toggleContextItem,
} from './agentic'

const preview = (): ContextPreview => ({
  total_bytes: 30,
  items: [
    {
      id: 'active',
      kind: 'active_file',
      path: 'src/lib.rs',
      label: 'src/lib.rs',
      content: 'abc',
      byte_len: 10,
      included: true,
      redacted: false,
      reason: 'active editor',
    },
    {
      id: 'tab',
      kind: 'open_tab',
      path: 'src/main.rs',
      label: 'src/main.rs',
      content: null,
      byte_len: 20,
      included: true,
      redacted: false,
      reason: 'open tab',
    },
  ],
  omitted: [],
})

const plan = (): AgentPlan => ({
  id: 'plan-1',
  title: 'Plan',
  assumptions: [],
  steps: [],
  likely_files: [],
  risks: [],
  suggested_validation: [],
  missing_context: [],
})

const session = (): AgentSession => ({
  id: 'sess-1',
  project_root: '/workspace',
  goal: 'Do the thing',
  phase: 'awaiting_plan_approval',
  interrupted: false,
  provider: 'anthropic',
  model: 'claude',
  tier: 'ask',
  context: preview(),
  plan: plan(),
  approvals: [],
  change_sets: [],
  apply_report: null,
  validation_runs: [],
  summary: null,
})

const changeSet = (): ChangeSet => ({
  id: 'cs-1',
  plan_id: 'plan-1',
  parse_warnings: [],
  files: [
    {
      id: 'file-1',
      old_path: 'src/lib.rs',
      new_path: 'src/lib.rs',
      change_kind: 'modify',
      approval: 'pending',
      hunks: [
        {
          id: 'hunk-1',
          old_start: 1,
          old_count: 1,
          new_start: 1,
          new_count: 1,
          header: '',
          lines: [{ kind: 'added', text: 'new line' }],
          approval: 'pending',
        },
      ],
    },
  ],
})

describe('agentic store', () => {
  beforeEach(() => resetAgent())

  it('mirrors a session snapshot into render state', () => {
    const s0 = session()
    s0.apply_report = { applied: [], rejected: [], skipped: [], failed: [] }
    s0.summary = {
      id: 'sum-1',
      goal: 'Do the thing',
      plan_title: 'Plan',
      changed_files: ['src/lib.rs'],
      applied_count: 1,
      rejected_count: 0,
      failed_count: 0,
      skipped_count: 0,
      validation_status: '1 passed',
      unresolved_risks: [],
      follow_ups: [],
      text: 'Done',
      local_fallback: true,
    }
    setSession(s0)
    const s = get(agentic)
    expect(s.phase).toBe('awaiting_plan_approval')
    expect(s.plan?.id).toBe('plan-1')
    expect(s.contextPreview?.items).toHaveLength(2)
    expect(s.applyReport).toEqual({ applied: [], rejected: [], skipped: [], failed: [] })
    expect(s.summary?.text).toBe('Done')
  })

  it('toggles context items and recomputes included bytes', () => {
    setContextPreview(preview())
    toggleContextItem('tab')
    const s = get(agentic)
    expect(includedContextIds(s.contextPreview)).toEqual(['active'])
    expect(s.contextPreview?.total_bytes).toBe(10)
  })

  it('detects an unconsumed plan approval', () => {
    setSession(session())
    expect(isPlanApproved(get(agentic))).toBe(false)
    addApproval({
      id: 'approval-1',
      kind: 'plan',
      target_id: 'plan-1',
      created_at: '2026-06-23T00:00:00Z',
      consumed: false,
    })
    expect(isPlanApproved(get(agentic))).toBe(true)
  })

  it('selects the latest change set and detects approved hunks', () => {
    const s = session()
    const cs = changeSet()
    s.change_sets = [{ ...cs, id: 'old' }, cs]
    setSession(s)
    expect(latestChangeSet(get(agentic))?.id).toBe('cs-1')
    expect(hasApprovedChange(latestChangeSet(get(agentic)))).toBe(false)

    cs.files[0].hunks[0].approval = 'approved'
    s.change_sets = [cs]
    setSession(s)
    expect(hasApprovedChange(latestChangeSet(get(agentic)))).toBe(true)
  })
})
