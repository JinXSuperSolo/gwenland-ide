import { get } from 'svelte/store'
import type { UnlistenFn } from '@tauri-apps/api/event'

import * as cmd from '../tauri/commands'
import { workspace } from '../stores/workspace'
import { tabs, isEditorTab } from '../stores/tabs'
import { activeSelection } from '../editor/active-editor'
import { aiChat, aiState, appendUserTurn, appendAgentTurn, snapshotAgentTurn, rollbackEmptyAgentTurn } from '../stores/ai-chat'
import { createConversation } from '../ai/ai-chat-setup'
import {
  agentic,
  agenticState,
  resetAgent,
  setSession,
  setPhase,
  setContextPreview,
  startAgentStream,
  appendAgentText,
  endAgentStream,
  addApproval,
  setAgentError,
  setAgentBusy,
  includedContextIds,
  latestChangeSet,
  hasDestructiveApprovedChange,
  startToolLoop,
  endToolLoop,
  addToolCall,
  addToolResult,
  setPendingTool,
  setPendingAsk,
  setToolFinal,
  setToolExhausted,
  setTierLocal,
  setRunningCommand,
  appendCmdOutputLine,
} from '../stores/agentic'

/**
 * Agent workflow orchestration (Milestone 10). Bridges the `agentic` store and
 * the engine-backed `agent_*` Tauri commands. The store stays Tauri-free; all
 * `invoke`/event wiring lives here. Streaming listeners are registered BEFORE
 * `agent_request_plan` is invoked so no chunk/phase event can be missed.
 *
 * Listener teardown is centralized in `teardownAgentListeners` and is safe to
 * call repeatedly (idempotent) — Requirement: "Keep runtime listeners
 * teardown-safe."
 */

let unlistenChunk: UnlistenFn | null = null
let unlistenPhase: UnlistenFn | null = null
let unlistenError: UnlistenFn | null = null

/** Tear down all agent stream listeners (idempotent). */
export function teardownAgentListeners(): void {
  unlistenChunk?.()
  unlistenChunk = null
  unlistenPhase?.()
  unlistenPhase = null
  unlistenError?.()
  unlistenError = null
}

// --- Tool-calling ReAct loop listeners (M10 Wave 7) ------------------------
// The loop is UI-driven and re-entrant: one `agent_tool_step` per call. These
// listeners stay alive for the whole loop (across gate pauses) and are torn
// down only when the loop finishes (final/exhausted/error) or is cancelled.

let unlistenToolChunk: UnlistenFn | null = null
let unlistenToolCall: UnlistenFn | null = null
let unlistenToolResult: UnlistenFn | null = null
let unlistenToolAsk: UnlistenFn | null = null
let unlistenToolError: UnlistenFn | null = null
let unlistenCmdOutput: UnlistenFn | null = null
let unlistenCmdDone: UnlistenFn | null = null
/** Stream id shared by every step of the active loop, or null when idle. */
let activeToolStreamId: string | null = null
/** Set by Stop/cancel to halt the re-entrant loop between steps (best-effort:
 * the in-flight `agent_tool_step` still completes). Matters most for the
 * Full-Control tier, which otherwise auto-runs many steps unattended. */
let toolLoopAbort = false

/** Tear down all tool-loop listeners (idempotent). */
export function teardownToolListeners(): void {
  unlistenToolChunk?.()
  unlistenToolChunk = null
  unlistenToolCall?.()
  unlistenToolCall = null
  unlistenToolResult?.()
  unlistenToolResult = null
  unlistenToolAsk?.()
  unlistenToolAsk = null
  unlistenToolError?.()
  unlistenToolError = null
  unlistenCmdOutput?.()
  unlistenCmdOutput = null
  unlistenCmdDone?.()
  unlistenCmdDone = null
  activeToolStreamId = null
  setRunningCommand(false)
}

/** Open project root, or null when no folder is open. */
function projectRoot(): string | null {
  return get(workspace).folderPath
}

/** Active editor tab's on-disk path, or null. */
function currentFilePath(): string | null {
  const s = get(tabs)
  const t = s.tabs.find((tab) => tab.id === s.activeId)
  return t && isEditorTab(t) && t.path ? t.path : null
}

/** All open editor-tab paths (for context candidates). */
function openTabPaths(): string[] {
  return get(tabs)
    .tabs.filter(isEditorTab)
    .map((t) => t.path)
    .filter((p): p is string => !!p)
}

/** Gather current workspace state as agent context input. */
function gatherContextInput(): cmd.AgentContextInput {
  const activeFile = currentFilePath()
  const selText = activeSelection()
  const selection = activeFile && selText ? { path: activeFile, content: selText } : null
  return {
    active_file: activeFile,
    selection,
    open_tabs: openTabPaths(),
  }
}

// --- Session lifecycle -----------------------------------------------------

/**
 * Start a new agent session for `goal`, then immediately build its context
 * preview. Uses the AI panel's active provider/model. No-op without an open
 * folder or a non-empty goal.
 */
export async function startAgentSession(goal: string): Promise<void> {
  const root = projectRoot()
  const trimmed = goal.trim()
  if (!root || !trimmed) return

  const { activeProvider, activeModel } = get(aiChat)
  const tier = agenticState().tier
  teardownAgentListeners()
  resetAgent()
  setAgentBusy(true)
  try {
    const session = await cmd.agentCreateSession(root, trimmed, activeProvider, activeModel, tier)
    setSession(session)
    await refreshContextPreview()
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
  } finally {
    setAgentBusy(false)
  }
}

/**
 * Chat-style entry point: start a session for `goal` and immediately draft a
 * plan, so submitting a goal in the composer streams a plan back like a chat
 * reply. `startAgentSession` resets any prior session first.
 */
export async function startAgentGoal(goal: string): Promise<void> {
  await startAgentSession(goal)
  if (agenticState().session) await requestPlan()
}

/**
 * Agent-mode entry point for the unified chat stream (M8): append the goal as a
 * user turn + an agent-run turn to the SAME conversation as chat, then run the
 * ReAct tool loop inline. The previous live run is frozen into its message first
 * so past agent turns stay in the single history.
 */
export async function startAgentRun(goal: string): Promise<void> {
  const trimmed = goal.trim()
  if (!trimmed) return

  // One unified history: agent runs live in the active conversation.
  if (!aiState().activeConversationId) {
    await createConversation()
    if (!aiState().activeConversationId) return // no project / creation failed
  }

  // Freeze the previous live agent run into its message before the engine resets.
  const prev = agenticState()
  if (prev.runId) {
    snapshotAgentTurn(prev.runId, {
      toolLog: prev.toolLog,
      final: prev.toolFinal,
      stopped: prev.toolStopped,
    })
  }

  // Show the goal immediately, then start the session + run.
  appendUserTurn(trimmed)
  await startAgentSession(trimmed)
  const runId = agenticState().runId
  if (!runId) return
  appendAgentTurn(runId)
  await runAgentTools()
}

/** Rebuild the context preview from current workspace state. */
export async function refreshContextPreview(): Promise<void> {
  const session = agenticState().session
  if (!session) return
  try {
    const preview = await cmd.agentContextPreview(session.id, gatherContextInput())
    setContextPreview(preview)
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
  }
}

/** Re-fetch the server session snapshot (picks up the normalized plan, etc.). */
async function refreshSession(): Promise<void> {
  const session = agenticState().session
  if (!session) return
  try {
    const fresh = await cmd.agentGetSession(session.id)
    setSession(fresh)
  } catch {
    /* leave the mirrored state as-is */
  }
}

/** Restore the most recent persisted session for the current workspace, if any. */
export async function restoreAgentSessions(projectRoot: string): Promise<void> {
  if (!projectRoot || agenticState().session) return
  setAgentBusy(true)
  try {
    const sessions = await cmd.agentRestoreSessions(projectRoot)
    if (sessions.length > 0 && !agenticState().session) {
      setSession(sessions[0])
    }
  } catch (e) {
    setAgentError({ kind: 'storage_error', data: String(e) })
  } finally {
    setAgentBusy(false)
  }
}

// --- Plan request / streaming ----------------------------------------------

/**
 * Request a plan for the active session. Registers listeners first, then
 * invokes `agent_request_plan`. Streamed text accumulates in the store; on the
 * `awaiting_plan_approval` phase event we re-fetch the session to pick up the
 * normalized plan.
 */
export async function requestPlan(): Promise<void> {
  const state = agenticState()
  const session = state.session
  if (!session || state.activeStreamId) return

  const streamId = crypto.randomUUID()
  const contextIds = includedContextIds(state.contextPreview)

  startAgentStream(streamId)
  teardownAgentListeners()

  unlistenChunk = await cmd.onAgentChunk(streamId, (text) => appendAgentText(streamId, text))
  unlistenPhase = await cmd.onAgentPhase(session.id, (phase) => {
    setPhase(phase)
    if (phase === 'awaiting_plan_approval') {
      endAgentStream(streamId)
      void refreshSession()
      teardownAgentListeners()
    } else if (phase === 'goal') {
      // Stream failed and reverted; allow a retry.
      endAgentStream(streamId)
    }
  })
  unlistenError = await cmd.onAgentError(session.id, (error) => {
    if (error.kind !== 'cancelled') setAgentError(error)
    endAgentStream(streamId)
    teardownAgentListeners()
  })

  try {
    await cmd.agentRequestPlan(session.id, streamId, contextIds)
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
    endAgentStream(streamId)
    teardownAgentListeners()
  }
}

/**
 * Ask for a revised plan: re-request planning. The server allows
 * `awaiting_plan_approval -> drafting_plan`, keeping the session in plan review
 * until a new plan is approved (Requirement 2.7).
 */
export async function revisePlan(): Promise<void> {
  await requestPlan()
}

// --- Approvals -------------------------------------------------------------

/** Approve the current plan, unlocking edit generation. */
export async function approvePlan(): Promise<void> {
  const state = agenticState()
  const session = state.session
  const plan = state.plan
  if (!session || !plan) return
  setAgentBusy(true)
  try {
    const record = await cmd.agentApprovePlan(session.id, plan.id)
    addApproval(record)
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
  } finally {
    setAgentBusy(false)
  }
}

/**
 * Request edits from the approved plan. Registers listeners first, then invokes
 * `agent_request_edits`; on `awaiting_edit_approval` we re-fetch the session to
 * pick up the parsed ChangeSet. No file writes happen in Wave 4.
 */
export async function requestEdits(): Promise<void> {
  const state = agenticState()
  const session = state.session
  if (!session || state.activeStreamId) return

  const streamId = crypto.randomUUID()
  const contextIds = includedContextIds(state.contextPreview)

  startAgentStream(streamId)
  teardownAgentListeners()

  unlistenChunk = await cmd.onAgentChunk(streamId, (text) => appendAgentText(streamId, text))
  unlistenPhase = await cmd.onAgentPhase(session.id, (phase) => {
    setPhase(phase)
    if (phase === 'awaiting_edit_approval') {
      endAgentStream(streamId)
      void refreshSession()
      teardownAgentListeners()
    } else if (phase === 'awaiting_plan_approval') {
      endAgentStream(streamId)
    }
  })
  unlistenError = await cmd.onAgentError(session.id, (error) => {
    if (error.kind !== 'cancelled') setAgentError(error)
    endAgentStream(streamId)
    teardownAgentListeners()
  })

  try {
    await cmd.agentRequestEdits(session.id, streamId, contextIds)
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
    endAgentStream(streamId)
    teardownAgentListeners()
  }
}

/** Set one hunk's review state without applying it. */
export async function setHunkApproval(
  changeSetId: string,
  hunkId: string,
  approval: cmd.ApprovalState
): Promise<void> {
  const session = agenticState().session
  if (!session) return
  setAgentBusy(true)
  try {
    setSession(await cmd.agentSetHunkApproval(session.id, changeSetId, hunkId, approval))
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
  } finally {
    setAgentBusy(false)
  }
}

/** Set every hunk in one file change to the same review state. */
export async function setFileApproval(
  changeSetId: string,
  fileId: string,
  approval: cmd.ApprovalState
): Promise<void> {
  const session = agenticState().session
  if (!session) return
  setAgentBusy(true)
  try {
    setSession(await cmd.agentSetFileApproval(session.id, changeSetId, fileId, approval))
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
  } finally {
    setAgentBusy(false)
  }
}

/** Apply approved changes after any required destructive confirmation. */
export async function applyApprovedChanges(): Promise<void> {
  const state = agenticState()
  const session = state.session
  const changeSet = latestChangeSet(state)
  if (!session || !changeSet) return

  let destructiveConfirmed = false
  if (hasDestructiveApprovedChange(changeSet)) {
    destructiveConfirmed = window.confirm(
      'This apply includes delete or rename changes. Apply the approved destructive changes?'
    )
    if (!destructiveConfirmed) return
  }

  setAgentBusy(true)
  try {
    setSession(await cmd.agentApplyChanges(session.id, destructiveConfirmed))
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
  } finally {
    setAgentBusy(false)
  }
}

/** Approve then run one validation command. The backend consumes the approval. */
export async function approveAndRunValidation(
  commandId: string,
  sizeImpactNote: string | null = null,
  dangerConfirmed = false
): Promise<void> {
  const session = agenticState().session
  if (!session) return
  setAgentBusy(true)
  try {
    const approval = await cmd.agentApproveValidationCommand(
      session.id,
      commandId,
      sizeImpactNote,
      dangerConfirmed
    )
    addApproval(approval)
    setSession(await cmd.agentRunValidation(session.id, commandId, approval.id))
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
  } finally {
    setAgentBusy(false)
  }
}

/** Build the final summary. No command or edit runs during summarization. */
export async function summarizeAgentSession(): Promise<void> {
  const session = agenticState().session
  if (!session) return
  setAgentBusy(true)
  try {
    setSession(await cmd.agentSummarize(session.id))
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
  } finally {
    setAgentBusy(false)
  }
}

/** Cancel/abandon the session (changes no files). */
export async function cancelAgentSession(): Promise<void> {
  const state = agenticState()
  const { session, runId } = state
  if (!session) {
    resetAgent()
    return
  }
  try {
    await cmd.agentCancel(session.id)
  } catch {
    /* cancelling a finished/missing session is a no-op */
  }
  toolLoopAbort = true
  teardownAgentListeners()
  teardownToolListeners()
  endToolLoop()

  // If the run produced no tool steps and no final answer, roll back the
  // user turn + agent turn so the conversation list stays clean.
  if (runId && !state.toolLog.length && !state.toolFinal) {
    rollbackEmptyAgentTurn(runId)
  } else {
    setPhase('cancelled')
  }
}

// --- Tool-calling ReAct loop (M10 Wave 7) ----------------------------------

/**
 * Drive the loop forward: call `agent_tool_step` repeatedly while it auto-runs
 * non-gated tools (`kind === 'ran'`), and stop when it finishes (`final` /
 * `exhausted`) or parks a gated tool / ask (`awaiting`). On `awaiting` the loop
 * returns and waits; `resolveToolGate` / `answerToolAsk` resume it.
 *
 * Listeners are registered once in `runAgentTools` and survive these pauses, so
 * this only loops and reads `activeToolStreamId`.
 */
async function driveToolLoop(sessionId: string): Promise<void> {
  const streamId = activeToolStreamId
  if (!streamId) return
  const contextIds = includedContextIds(agenticState().contextPreview)

  setAgentBusy(true)
  try {
    for (;;) {
      if (toolLoopAbort) {
        teardownToolListeners()
        return
      }
      // Reset the per-turn reasoning buffer so the panel shows the live turn.
      startAgentStream(streamId)
      const res = await cmd.agentToolStep(sessionId, streamId, contextIds)
      endAgentStream(streamId)

      if (toolLoopAbort) {
        teardownToolListeners()
        return
      }
      if (res.kind === 'ran') continue
      if (res.kind === 'final') {
        setToolFinal(res.text)
        teardownToolListeners()
        return
      }
      if (res.kind === 'exhausted') {
        setToolExhausted()
        teardownToolListeners()
        return
      }
      // res.kind === 'awaiting': park and wait for the user. `ask_user` carries
      // its prompt via the `agent://ask` event, which the listener turns into
      // `pendingAsk`; only mutating/terminal gates are set from here.
      if (res.side !== 'ask') {
        setPendingTool({ id: res.id, tool: res.tool, side: res.side, risk: res.risk })
      }
      return
    }
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
    endToolLoop()
    teardownToolListeners()
  } finally {
    setAgentBusy(false)
  }
}

/**
 * Start the tool-calling ReAct loop for the active session. Registers tool
 * listeners first (so no `tool_call`/`tool_result`/`ask` event is missed), then
 * drives the loop until it finishes or hits the first gate. No-op without a
 * session or while a loop is already active.
 */
export async function runAgentTools(): Promise<void> {
  const session = agenticState().session
  if (!session || activeToolStreamId) return

  teardownToolListeners()
  startToolLoop()
  toolLoopAbort = false
  const streamId = crypto.randomUUID()
  activeToolStreamId = streamId

  unlistenToolChunk = await cmd.onAgentChunk(streamId, (text) => appendAgentText(streamId, text))
  unlistenToolCall = await cmd.onAgentToolCall(session.id, (call) =>
    addToolCall({ id: call.id, tool: call.tool, args: call.args })
  )
  unlistenToolResult = await cmd.onAgentToolResult(session.id, (r) =>
    addToolResult({ id: r.id, ok: r.ok, content: r.content, error: r.error })
  )
  unlistenToolAsk = await cmd.onAgentAsk(session.id, (ask) =>
    setPendingAsk({ id: ask.id, prompt: ask.prompt, options: ask.options, multi: ask.multi })
  )
  unlistenToolError = await cmd.onAgentError(session.id, (error) => {
    if (error.kind !== 'cancelled') setAgentError(error)
    endToolLoop()
    teardownToolListeners()
    setAgentBusy(false)
  })
  unlistenCmdOutput = await cmd.onAgentCmdOutput(session.id, (line) =>
    appendCmdOutputLine(line)
  )
  unlistenCmdDone = await cmd.onAgentCmdDone(session.id, () =>
    setRunningCommand(false)
  )

  await driveToolLoop(session.id)
}

/**
 * Resolve a parked mutating/terminal gate, then resume the loop. `decision` is
 * `'approve'` for normal actions, `'confirm'` for destructive ones (delete /
 * destructive command), or `'reject'` to decline. The backend treats
 * approve/confirm alike; the distinction is the UI's destructive guard.
 */
export async function resolveToolGate(
  decision: 'approve' | 'confirm' | 'reject'
): Promise<void> {
  const state = agenticState()
  const session = state.session
  if (!session || !state.pendingTool) return
  const isTerminal = state.pendingTool.side === 'terminal'
  setPendingTool(null)
  // Mark a running command so the UI shows the live output banner.
  if (decision !== 'reject' && isTerminal) setRunningCommand(true)
  setAgentBusy(true)
  try {
    await cmd.agentToolResolve(session.id, decision, [])
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
    setRunningCommand(false)
    setAgentBusy(false)
    return
  }
  await driveToolLoop(session.id)
}

/** Answer a parked `ask_user` prompt with the chosen option(s), then resume. */
export async function answerToolAsk(selection: string[]): Promise<void> {
  const session = agenticState().session
  if (!session || !agenticState().pendingAsk) return
  setPendingAsk(null)
  setAgentBusy(true)
  try {
    await cmd.agentToolResolve(session.id, 'approve', selection)
  } catch (e) {
    setAgentError({ kind: 'provider_error', data: String(e) })
    setAgentBusy(false)
    return
  }
  await driveToolLoop(session.id)
}

/**
 * Change the autonomy tier (M10 Wave 8). Updates the store optimistically; if a
 * session exists, persists it server-side (allowed only between iterations) and
 * adopts the confirmed snapshot, reverting the optimistic change on refusal.
 */
export async function changeAgentTier(tier: cmd.AgentTier): Promise<void> {
  const previous = agenticState().tier
  setTierLocal(tier)
  const session = agenticState().session
  if (!session) return
  try {
    setSession(await cmd.agentSetTier(session.id, tier))
  } catch (e) {
    setTierLocal(previous)
    setAgentError({ kind: 'provider_error', data: String(e) })
  }
}

/** Subscribe nothing globally; this is here for symmetry/future use. */
export { agentic }
