import type { PendingAsk, PendingToolGate, ToolLogEntry } from './stores/agentic'

/**
 * Provider-independent agent-activity model (M8). Translates the engine's raw
 * tool log + gate state into safe, user-facing activity summaries for the chat
 * pane — never raw reasoning / hidden chain-of-thought. Pure + testable; the UI
 * (ActivityRow / the active row) renders these, and a dev mock can synthesize
 * them without any provider.
 */

export type ActivityKind =
  | 'thinking'
  | 'read_file'
  | 'search'
  | 'edit_file'
  | 'write_file'
  | 'run_command'
  | 'approval'
  | 'done'
  | 'error'

export type ActivityStatus = 'running' | 'ok' | 'failed' | 'pending'

/** A run's lifecycle state, for clean idle/active/completed/failed/cancelled UI. */
export type RunState = 'idle' | 'active' | 'completed' | 'failed' | 'cancelled'

export interface AgentActivity {
  id: string
  kind: ActivityKind
  /** Verb-phrase for the active row / timeline, e.g. "Reading src/main.rs". */
  label: string
  /** Optional secondary summary (result preview, exit status). Safe to show. */
  detail?: string
  status: ActivityStatus
}

/** Read one string arg out of a tool call's JSON args (best-effort, never throws). */
function argStr(argsJson: string, key: string): string {
  try {
    const o = JSON.parse(argsJson) as Record<string, unknown>
    return typeof o[key] === 'string' ? (o[key] as string) : ''
  } catch {
    return ''
  }
}

/** Map a raw tool name to its activity kind. */
export function kindForTool(tool: string): ActivityKind {
  switch (tool) {
    case 'read_file':
    case 'get_git_diff':
    case 'get_diagnostics':
      return 'read_file'
    case 'list_dir':
    case 'grep_search':
    case 'file_search':
      return 'search'
    case 'edit_file':
      return 'edit_file'
    case 'write_file':
      return 'write_file'
    case 'delete_file':
      return 'write_file'
    case 'run_terminal_cmd':
    case 'open_browser':
      return 'run_command'
    case 'ask_user':
      return 'approval'
    default:
      return 'thinking'
  }
}

/** A short, present-tense label for a tool call (target included when known). */
function labelForTool(tool: string, argsJson: string): string {
  const path = argStr(argsJson, 'path')
  const query = argStr(argsJson, 'query')
  const command = argStr(argsJson, 'command')
  switch (tool) {
    case 'read_file':
      return path ? `Reading ${path}` : 'Reading file'
    case 'list_dir':
      return path ? `Listing ${path}` : 'Listing directory'
    case 'grep_search':
      return query ? `Searching “${query}”` : 'Searching'
    case 'file_search':
      return query ? `Finding “${query}”` : 'Finding files'
    case 'get_git_diff':
      return 'Reading git diff'
    case 'get_diagnostics':
      return 'Reading diagnostics'
    case 'edit_file':
      return path ? `Editing ${path}` : 'Editing file'
    case 'write_file':
      return path ? `Writing ${path}` : 'Writing file'
    case 'delete_file':
      return path ? `Deleting ${path}` : 'Deleting file'
    case 'run_terminal_cmd':
      return command ? `Running ${command}` : 'Running command'
    case 'open_browser':
      return 'Opening browser'
    case 'ask_user':
      return 'Asking a question'
    default:
      return 'Working'
  }
}

/** A short result/summary line for a finished call (first content line / error). */
function detailForResult(entry: ToolLogEntry): string | undefined {
  if (entry.ok === false) return entry.error ?? 'failed'
  if (entry.ok !== true || !entry.content) return undefined
  const kind = kindForTool(entry.tool)
  if (kind === 'search') {
    const first = entry.content.split('\n').find((l) => l.trim())
    return first ? `→ ${first.trim()}` : undefined
  }
  if (entry.tool === 'run_terminal_cmd') {
    const first = entry.content.split('\n').find((l) => l.trim())
    return first ? first.trim() : undefined
  }
  return undefined
}

/** Translate one tool-log entry into a user-facing activity. */
export function activityFromToolEntry(entry: ToolLogEntry): AgentActivity {
  const status: ActivityStatus =
    entry.ok === null ? 'running' : entry.ok ? 'ok' : 'failed'
  return {
    id: entry.id,
    kind: kindForTool(entry.tool),
    label: labelForTool(entry.tool, entry.args),
    detail: detailForResult(entry),
    status,
  }
}

/** The whole timeline for a run. */
export function deriveActivities(toolLog: ToolLogEntry[]): AgentActivity[] {
  return toolLog.map(activityFromToolEntry)
}

export interface RunSnapshotInput {
  toolLog: ToolLogEntry[]
  pendingTool: PendingToolGate | null
  pendingAsk: PendingAsk | null
  busy: boolean
}

/** The pending approval as an activity, or null when nothing is awaiting. */
export function pendingActivity(input: RunSnapshotInput): AgentActivity | null {
  const { pendingTool, pendingAsk, toolLog } = input
  if (pendingAsk) {
    return { id: pendingAsk.id, kind: 'approval', label: 'Waiting for your choice', status: 'pending' }
  }
  if (pendingTool) {
    const entry = toolLog.find((e) => e.id === pendingTool.id)
    const what = entry ? labelForTool(entry.tool, entry.args) : pendingTool.tool
    const verb = pendingTool.side === 'terminal' ? 'Approve' : 'Review'
    return { id: pendingTool.id, kind: 'approval', label: `Waiting for approval — ${verb} ${what}`, status: 'pending' }
  }
  return null
}

/**
 * The compact active-row label: the pending approval, else the in-flight step,
 * else a plain "Thinking" while the engine works. Returns null when idle.
 */
export function currentActivityLabel(input: RunSnapshotInput): string | null {
  const pending = pendingActivity(input)
  if (pending) return pending.label
  const running = [...input.toolLog].reverse().find((e) => e.ok === null)
  if (running) return labelForTool(running.tool, running.args)
  if (input.busy) return 'Thinking'
  return null
}

export interface RunStateInput {
  active: boolean
  busy: boolean
  stopped: 'final' | 'exhausted' | null
  hasError: boolean
  cancelled: boolean
}

/** Lifecycle state of a run for idle/active/completed/failed/cancelled UI. */
export function runState(input: RunStateInput): RunState {
  if (input.cancelled) return 'cancelled'
  if (input.hasError) return 'failed'
  if (input.stopped) return 'completed'
  if (input.active || input.busy) return 'active'
  return 'idle'
}
