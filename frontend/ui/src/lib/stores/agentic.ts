import { writable, get } from 'svelte/store'
import type {
  AgentPhase,
  AgentPlan,
  AgentSession,
  AgentSummary,
  AgentTier,
  AiError,
  ApprovalState,
  ApprovalRecord,
  ApplyReport,
  ChangeSet,
  ContextPreview,
  ValidationRun,
} from '../tauri/commands'

/**
 * Agentic workflow panel state (Milestone 10). Holds everything the agent UI
 * renders for one session: the session + phase, the context preview, the live
 * streamed text (plan/edit drafting), the normalized plan, recorded approvals,
 * proposed change sets, validation runs, and the final summary.
 *
 * Like `ai-chat`, this store is Tauri-free — all `invoke`/event wiring lives in
 * `../agentic/agentic-setup.ts`. It deliberately stores NO API keys or secret
 * file contents (Requirement 8.3): the session it mirrors carries only
 * provider/model ids, and context item contents are already secret-redacted
 * engine-side before they reach here.
 */

/**
 * One entry in the tool-loop activity log (M10 Wave 7): a tool call and — once
 * the observation lands — its result. Args/content are already secret-redacted
 * and size-bounded on the Tauri side before they reach the store.
 */
export interface ToolLogEntry {
  id: string
  tool: string
  /** JSON-encoded args as emitted by the model. */
  args: string
  /** null while the tool is still running; true/false once observed. */
  ok: boolean | null
  /** Observation content, or null while pending. */
  content: string | null
  error: string | null
}

/** A mutating/terminal tool parked awaiting an Apply/Validation gate decision. */
export interface PendingToolGate {
  id: string
  tool: string
  /** 'mutating' | 'terminal' — picks which gate language to show. */
  side: string
  /** `CommandRisk` debug string for terminal runs, else null. */
  risk: string | null
}

/** An `ask_user` prompt paused waiting on the user's A/B/C/D choice. */
export interface PendingAsk {
  id: string
  prompt: string
  options: string[]
  multi: boolean
}

export interface AgenticState {
  /** The active session, or null before one is started. */
  session: AgentSession | null
  /** Current phase (mirrors `session.phase`; updated live from `agent://phase`). */
  phase: AgentPhase | null
  /** Policy-filtered context preview for the session. */
  contextPreview: ContextPreview | null
  /** Buffer of the in-flight plan/edit stream text. */
  streamedText: string
  /** Stream id of the in-flight request, or null. */
  activeStreamId: string | null
  /** Normalized plan once drafting completes. */
  plan: AgentPlan | null
  /** Recorded approvals (plan/edits/validation), one-use server-side. */
  approvals: ApprovalRecord[]
  /** Proposed change sets (Wave 4+). */
  changeSets: ChangeSet[]
  /** Latest apply result (Wave 5+). */
  applyReport: ApplyReport | null
  /** Validation runs (Wave 5+). */
  validationRuns: ValidationRun[]
  /** Final summary (Wave 6). */
  summary: AgentSummary | null
  /** Last recoverable error (cancellation does not set this). */
  lastError: AiError | null
  /** True while a command/stream is in flight (drives spinners + disabled). */
  busy: boolean
  // --- Tool-calling ReAct loop (M10 Wave 7) --------------------------------
  /** Ordered activity log of tool calls and their observations. */
  toolLog: ToolLogEntry[]
  /** A mutating/terminal tool parked at its gate, or null. */
  pendingTool: PendingToolGate | null
  /** An `ask_user` prompt awaiting selection, or null. */
  pendingAsk: PendingAsk | null
  /** The loop's final answer once it finishes, or null. */
  toolFinal: string | null
  /** How the loop ended, or null while idle/running. */
  toolStopped: 'final' | 'exhausted' | null
  /** True while a tool loop is in progress (running OR paused at a gate). */
  toolActive: boolean
  /** Autonomy tier for the next/active session (mirrors `session.tier`). */
  tier: AgentTier
  /** Id of the live run, matched against a message's `agent.runId` to tell
   *  which agent message in the unified stream renders from this live engine. */
  runId: string | null
}

const initial: AgenticState = {
  session: null,
  phase: null,
  contextPreview: null,
  streamedText: '',
  activeStreamId: null,
  plan: null,
  approvals: [],
  changeSets: [],
  applyReport: null,
  validationRuns: [],
  summary: null,
  lastError: null,
  busy: false,
  toolLog: [],
  pendingTool: null,
  pendingAsk: null,
  toolFinal: null,
  toolStopped: null,
  toolActive: false,
  tier: 'ask',
  runId: null,
}

export const agentic = writable<AgenticState>(initial)

/** Convenience read of the current snapshot. */
export function agenticState(): AgenticState {
  return get(agentic)
}

/** Reset to a clean slate (new session / abandon), keeping the chosen tier. */
export function resetAgent(): void {
  agentic.update((s) => ({ ...initial, tier: s.tier }))
}

/** Optimistically set the tier (mirrors `session.tier` once the server confirms). */
export function setTierLocal(tier: AgentTier): void {
  agentic.update((s) => ({ ...s, tier }))
}

/** Set the live run id (used by the unified stream + the dev mock driver). */
export function setRunId(runId: string | null): void {
  agentic.update((s) => ({ ...s, runId }))
}

/**
 * Adopt a fresh session snapshot, syncing the mirrored fields (phase, plan,
 * context, approvals, change sets, runs, summary) from it. Used after create and
 * after every server-side mutation we re-fetch.
 */
export function setSession(session: AgentSession): void {
  agentic.update((s) => ({
    ...s,
    session,
    phase: session.phase,
    tier: session.tier,
    runId: session.id,
    contextPreview: session.context,
    plan: session.plan,
    approvals: session.approvals,
    changeSets: session.change_sets,
    applyReport: session.apply_report,
    validationRuns: session.validation_runs,
    summary: session.summary,
  }))
}

/** Update just the phase (from a live `agent://phase` event). */
export function setPhase(phase: AgentPhase): void {
  agentic.update((s) => ({
    ...s,
    phase,
    // Keep the mirrored session phase coherent for derived checks.
    session: s.session ? { ...s.session, phase } : s.session,
  }))
}

/** Replace the context preview (after `agent_context_preview`). */
export function setContextPreview(preview: ContextPreview): void {
  agentic.update((s) => ({ ...s, contextPreview: preview }))
}

/** Recompute `total_bytes` from currently-included items. */
function recomputeTotal(preview: ContextPreview): ContextPreview {
  const total = preview.items.reduce((sum, i) => (i.included ? sum + i.byte_len : sum), 0)
  return { ...preview, total_bytes: total }
}

/**
 * Toggle whether a context item is included in the next request (Requirement
 * 3.7). Secret/oversized items live in `omitted` and are never in `items`, so
 * this only ever flips safe candidates.
 */
export function toggleContextItem(id: string): void {
  agentic.update((s) => {
    if (!s.contextPreview) return s
    const items = s.contextPreview.items.map((i) =>
      i.id === id ? { ...i, included: !i.included } : i
    )
    return { ...s, contextPreview: recomputeTotal({ ...s.contextPreview, items }) }
  })
}

/** The ids of currently-included context items (drives the plan request). */
export function includedContextIds(preview: ContextPreview | null): string[] {
  if (!preview) return []
  return preview.items.filter((i) => i.included).map((i) => i.id)
}

/** Begin a stream: record its id and clear the text buffer. */
export function startAgentStream(streamId: string): void {
  agentic.update((s) => ({ ...s, activeStreamId: streamId, streamedText: '', lastError: null }))
}

/** Append a streamed token to the buffer (only for the active stream). */
export function appendAgentText(streamId: string, text: string): void {
  if (!text) return
  agentic.update((s) =>
    s.activeStreamId === streamId ? { ...s, streamedText: s.streamedText + text } : s
  )
}

/** Clear the active stream marker (stream ended). */
export function endAgentStream(streamId: string): void {
  agentic.update((s) => (s.activeStreamId === streamId ? { ...s, activeStreamId: null } : s))
}

/** Record a plan (after normalization). */
export function setPlan(plan: AgentPlan | null): void {
  agentic.update((s) => ({ ...s, plan }))
}

/** Append an approval record. */
export function addApproval(record: ApprovalRecord): void {
  agentic.update((s) => ({ ...s, approvals: [...s.approvals, record] }))
}

/** Replace ChangeSets directly when a review command returns updated session data. */
export function setChangeSets(changeSets: ChangeSet[]): void {
  agentic.update((s) => ({ ...s, changeSets }))
}

/** Set / clear the recoverable error banner. */
export function setAgentError(error: AiError | null): void {
  agentic.update((s) => ({ ...s, lastError: error }))
}
export function clearAgentError(): void {
  setAgentError(null)
}

/** Toggle the busy flag. */
export function setAgentBusy(busy: boolean): void {
  agentic.update((s) => ({ ...s, busy }))
}

// --- Tool-calling ReAct loop (M10 Wave 7) ----------------------------------

/** Begin a fresh tool loop: clear the log, pending gates, and final answer. */
export function startToolLoop(): void {
  agentic.update((s) => ({
    ...s,
    toolLog: [],
    pendingTool: null,
    pendingAsk: null,
    toolFinal: null,
    toolStopped: null,
    toolActive: true,
  }))
}

/** Append a tool call to the log (ignores duplicates by id). */
export function addToolCall(call: { id: string; tool: string; args: string }): void {
  agentic.update((s) => {
    if (s.toolLog.some((e) => e.id === call.id)) return s
    const entry: ToolLogEntry = {
      id: call.id,
      tool: call.tool,
      args: call.args,
      ok: null,
      content: null,
      error: null,
    }
    return { ...s, toolLog: [...s.toolLog, entry] }
  })
}

/** Fill in the observation for a previously-logged tool call. */
export function addToolResult(result: {
  id: string
  ok: boolean
  content: string
  error: string | null
}): void {
  agentic.update((s) => ({
    ...s,
    toolLog: s.toolLog.map((e) =>
      e.id === result.id
        ? { ...e, ok: result.ok, content: result.content, error: result.error }
        : e
    ),
  }))
}

/** Park / clear a mutating or terminal tool at its gate. */
export function setPendingTool(gate: PendingToolGate | null): void {
  agentic.update((s) => ({ ...s, pendingTool: gate }))
}

/** Park / clear an `ask_user` prompt. */
export function setPendingAsk(ask: PendingAsk | null): void {
  agentic.update((s) => ({ ...s, pendingAsk: ask }))
}

/** Record the loop's final answer and stop it. */
export function setToolFinal(text: string): void {
  agentic.update((s) => ({
    ...s,
    toolFinal: text,
    toolStopped: 'final',
    toolActive: false,
    pendingTool: null,
    pendingAsk: null,
  }))
}

/** Mark the loop as stopped on the iteration cap. */
export function setToolExhausted(): void {
  agentic.update((s) => ({
    ...s,
    toolStopped: 'exhausted',
    toolActive: false,
    pendingTool: null,
    pendingAsk: null,
  }))
}

/** End the loop without a final answer (e.g. on error). */
export function endToolLoop(): void {
  agentic.update((s) => ({ ...s, toolActive: false }))
}

// --- Derived selectors (pure) ----------------------------------------------

/**
 * Whether a parked gate is destructive and so needs an extra confirmation
 * rather than a one-click approve: deletes, plus terminal commands the policy
 * classified as Destructive / DependencyChanging / Blocked.
 */
export function isDestructiveGate(gate: PendingToolGate | null): boolean {
  if (!gate) return false
  if (gate.tool === 'delete_file') return true
  if (gate.side === 'terminal') {
    return ['Destructive', 'DependencyChanging', 'Blocked'].includes(gate.risk ?? '')
  }
  return false
}


/** True once the current plan has an (unconsumed) approval recorded. */
export function isPlanApproved(state: AgenticState): boolean {
  const planId = state.plan?.id
  if (!planId) return false
  return state.approvals.some((a) => a.kind === 'plan' && a.target_id === planId && !a.consumed)
}

/** Latest proposed ChangeSet, if edit generation has completed. */
export function latestChangeSet(state: AgenticState): ChangeSet | null {
  return state.changeSets.at(-1) ?? null
}

/** True if a ChangeSet has at least one approved file or hunk. */
export function hasApprovedChange(changeSet: ChangeSet | null): boolean {
  if (!changeSet) return false
  return changeSet.files.some(
    (file) =>
      file.approval === 'approved' ||
      file.hunks.some((hunk) => hunk.approval === ('approved' satisfies ApprovalState))
  )
}

/** True if an approved file/hunk belongs to a destructive file operation. */
export function hasDestructiveApprovedChange(changeSet: ChangeSet | null): boolean {
  if (!changeSet) return false
  return changeSet.files.some(
    (file) =>
      (file.change_kind === 'delete' || file.change_kind === 'rename') &&
      (file.approval === 'approved' || file.hunks.some((hunk) => hunk.approval === 'approved'))
  )
}

/** Whether the session is in a terminal phase. */
export function isTerminalPhase(phase: AgentPhase | null): boolean {
  return phase === 'complete' || phase === 'failed' || phase === 'cancelled'
}
