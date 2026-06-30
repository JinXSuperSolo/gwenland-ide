import { writable, get } from 'svelte/store'
import { collapseSidebar, showAgentTab } from './sidebar'
import type {
  AiError,
  ContextAttachment,
  ConversationMeta,
  DiffFile,
  GenericProviderSetting,
  ImageAttachment,
  ModelInfo,
  TerminalError,
} from '../tauri/commands'
import type { ToolLogEntry } from './agentic'

/**
 * AI chat panel state (Milestone 4). Replaces the M2.5 open/close-only stub.
 *
 * Holds everything the AI panel renders: panel visibility, the conversation
 * list + active conversation, the active conversation's messages, the in-flight
 * stream id, provider/model selection, key status, the last error banner, a
 * pending terminal error offered for "explain", and unsent composer text
 * (preserved across collapse/restore — Requirement 12.8).
 *
 * The store is Tauri-free: all backend calls live in `../ai/ai-chat-setup.ts`.
 */

export type MessageRole = 'user' | 'assistant'

/** Reasoning effort for thinking-capable models (Requirement 6.1). */
export type ReasoningLevel = 'low' | 'medium' | 'high' | 'extra_high'

/**
 * Runtime thinking/reasoning trace for an assistant message (Requirement 8).
 * This is display-only metadata — it is NOT persisted to conversation JSONL;
 * on reload it is reconstructed by parsing `<think>` markers out of the stored
 * content (see `splitThinking`).
 */
export interface ThinkingState {
  content: string
  streaming: boolean
  startedAt: number | null
  endedAt: number | null
  collapsed: boolean
}

/** A parsed unified-diff proposal detected in an assistant message (Req 10). */
export interface DiffProposal {
  files: DiffFile[]
  fileCount: number
  hunkCount: number
}

/**
 * A frozen view of a finished agent run (M8 unified history). Once a run is no
 * longer the live one, its message renders from this snapshot instead of the
 * live `agentic` engine, so past agent turns stay in the single chat stream.
 */
export interface AgentRunSnapshot {
  toolLog: ToolLogEntry[]
  final: string | null
  stopped: 'final' | 'exhausted' | null
}

/**
 * Marks an assistant message as an agent run. While `snapshot` is null and
 * `runId` matches the live `agentic` engine, the message renders live (tool
 * lines + inline gates); otherwise it renders the frozen snapshot.
 */
export interface AgentRunRef {
  runId: string
  snapshot: AgentRunSnapshot | null
}

export interface ChatMessage {
  id: string
  role: MessageRole
  content: string
  /** True while an assistant message is still streaming. */
  streaming: boolean
  timestamp: string
  attachments?: ContextAttachment[]
  /** Images sent with this (user) turn, for display only. */
  images?: ImageAttachment[]
  /** Separated reasoning trace, when the model produced one. */
  thinking?: ThinkingState
  /** Parsed diff proposal when the answer contained a usable unified diff. */
  diff?: DiffProposal
  /** Non-destructive notice when text looked like a diff but didn't parse. */
  diffError?: string
  /** Present when this assistant turn is an agent run (M8 unified history). */
  agent?: AgentRunRef
}

export interface AiChatState {
  isOpen: boolean
  conversations: ConversationMeta[]
  activeConversationId: string | null
  messages: ChatMessage[]
  activeStreamId: string | null
  activeProvider: string
  activeModel: string
  /** Reasoning effort applied to the next request on thinking-capable models. */
  reasoningLevel: ReasoningLevel
  /** User-configured generic OpenAI-compatible providers (for the picker). */
  genericProviders: GenericProviderSetting[]
  /** Models available for the active provider (null = not loaded / unsupported). */
  models: ModelInfo[] | null
  /** Whether a key is stored for the active provider. */
  hasKey: boolean
  lastError: AiError | null
  pendingTerminalError: TerminalError | null
  /** Composer text, preserved across panel collapse/restore. */
  unsentInput: string
}

const initial: AiChatState = {
  isOpen: false,
  conversations: [],
  activeConversationId: null,
  messages: [],
  activeStreamId: null,
  activeProvider: 'anthropic',
  activeModel: '',
  reasoningLevel: 'medium',
  genericProviders: [],
  models: null,
  hasKey: false,
  lastError: null,
  pendingTerminalError: null,
  unsentInput: '',
}

export const aiChat = writable<AiChatState>(initial)

/** Toggle panel visibility (bound to the status-bar AI button). */
export function toggleAiChat(): void {
  if (get(aiChat).isOpen) {
    collapseSidebar()
    aiChat.update((s) => ({ ...s, isOpen: false }))
  } else {
    showAgentTab()
    aiChat.update((s) => ({ ...s, isOpen: true }))
  }
}

/** Alias matching the design doc's name. */
export const toggleAiPanel = toggleAiChat

export function openAiChat(): void {
  showAgentTab()
  aiChat.update((s) => ({ ...s, isOpen: true }))
}

// --- Streaming mutations ---------------------------------------------------

/** Mark a streaming thinking trace complete and auto-collapse it (Req 8.6). */
function closeThinking(t: ThinkingState | undefined): ThinkingState | undefined {
  if (t && t.streaming) {
    return { ...t, streaming: false, endedAt: t.endedAt ?? Date.now(), collapsed: true }
  }
  return t
}

/** Append streamed reasoning text to the in-flight assistant message (Req 7.1). */
export function appendThinking(streamId: string, text: string): void {
  if (!text) return
  aiChat.update((s) => {
    if (s.activeStreamId !== streamId) return s
    const msgs = [...s.messages]
    const last = msgs[msgs.length - 1]
    if (last && last.role === 'assistant' && last.streaming) {
      const prev = last.thinking
      const thinking: ThinkingState = prev
        ? { ...prev, content: prev.content + text, streaming: true }
        : { content: text, streaming: true, startedAt: Date.now(), endedAt: null, collapsed: false }
      msgs[msgs.length - 1] = { ...last, thinking }
    }
    return { ...s, messages: msgs }
  })
}

/** Append a streamed answer token to the in-flight assistant message. */
export function appendToken(streamId: string, text: string): void {
  if (!text) return
  aiChat.update((s) => {
    if (s.activeStreamId !== streamId) return s
    const msgs = [...s.messages]
    const last = msgs[msgs.length - 1]
    if (last && last.role === 'assistant' && last.streaming) {
      // The first answer token means thinking is done → collapse it (Req 8.6).
      msgs[msgs.length - 1] = {
        ...last,
        content: last.content + text,
        thinking: closeThinking(last.thinking),
      }
    }
    return { ...s, messages: msgs }
  })
}

/** Mark the stream complete (clears the streaming flag + active stream id). */
export function finaliseStream(streamId: string): void {
  aiChat.update((s) => {
    if (s.activeStreamId !== streamId) return s
    const msgs = s.messages.map((m) =>
      m.streaming ? { ...m, streaming: false, thinking: closeThinking(m.thinking) } : m
    )
    return { ...s, messages: msgs, activeStreamId: null }
  })
}

/**
 * Record a stream error. Cancellation keeps partial text but shows NO banner
 * (Requirement 11.10 / 19.8); other errors set `lastError`.
 */
export function setStreamError(streamId: string, error: AiError): void {
  aiChat.update((s) => {
    if (s.activeStreamId !== streamId) return s

    // On cancel: if the streaming assistant message is still empty (no content
    // was received), roll back both the blank assistant message and its preceding
    // user message so the conversation list stays clean.
    if (error.kind === 'cancelled') {
      const streamingIdx = s.messages.findLastIndex((m) => m.streaming)
      if (streamingIdx !== -1 && !s.messages[streamingIdx].content.trim()) {
        const msgs = s.messages.filter((_, i) => {
          // Drop the blank streaming assistant and the user turn just before it.
          if (i === streamingIdx) return false
          if (i === streamingIdx - 1 && s.messages[i].role === 'user') return false
          return true
        })
        return { ...s, messages: msgs, activeStreamId: null, lastError: s.lastError }
      }
      // Has partial content — keep it, just stop streaming.
      const msgs = s.messages.map((m) =>
        m.streaming ? { ...m, streaming: false, thinking: closeThinking(m.thinking) } : m
      )
      return { ...s, messages: msgs, activeStreamId: null, lastError: s.lastError }
    }

    const msgs = s.messages.map((m) =>
      m.streaming ? { ...m, streaming: false, thinking: closeThinking(m.thinking) } : m
    )
    return { ...s, messages: msgs, activeStreamId: null, lastError: error }
  })
}

// --- Banner / input helpers ------------------------------------------------

export function clearError(): void {
  aiChat.update((s) => ({ ...s, lastError: null }))
}

export function setUnsentInput(text: string): void {
  aiChat.update((s) => ({ ...s, unsentInput: text }))
}

/** Set the reasoning effort for the next request (Requirement 6.7). */
export function setReasoningLevel(level: ReasoningLevel): void {
  aiChat.update((s) => ({ ...s, reasoningLevel: level }))
}

/** Attach a detected diff proposal (or non-destructive notice) to a message. */
export function setMessageDiff(
  messageId: string,
  result: { diff?: DiffProposal; diffError?: string }
): void {
  aiChat.update((s) => ({
    ...s,
    messages: s.messages.map((m) =>
      m.id === messageId ? { ...m, diff: result.diff, diffError: result.diffError } : m
    ),
  }))
}

// --- Agent runs in the unified stream (M8) ---------------------------------

/** Append a user turn (used by both chat and agent submits). Returns its id. */
export function appendUserTurn(
  content: string,
  attachments?: ContextAttachment[],
  images?: ImageAttachment[]
): string {
  const id = crypto.randomUUID()
  aiChat.update((s) => ({
    ...s,
    lastError: null,
    unsentInput: '',
    messages: [
      ...s.messages,
      {
        id,
        role: 'user',
        content,
        streaming: false,
        timestamp: new Date().toISOString(),
        attachments: attachments?.length ? attachments : undefined,
        images: images?.length ? images : undefined,
      },
    ],
  }))
  return id
}

/** Append an assistant agent-run turn linked to `runId`. Returns its id. */
export function appendAgentTurn(runId: string): string {
  const id = crypto.randomUUID()
  aiChat.update((s) => ({
    ...s,
    messages: [
      ...s.messages,
      {
        id,
        role: 'assistant',
        content: '',
        streaming: false,
        timestamp: new Date().toISOString(),
        agent: { runId, snapshot: null },
      },
    ],
  }))
  return id
}

/** Freeze the live agent message for `runId` so it renders from the snapshot. */
export function snapshotAgentTurn(runId: string, snapshot: AgentRunSnapshot): void {
  aiChat.update((s) => ({
    ...s,
    messages: s.messages.map((m) =>
      m.agent && m.agent.runId === runId && m.agent.snapshot === null
        ? { ...m, agent: { ...m.agent, snapshot } }
        : m
    ),
  }))
}

/**
 * Roll back a cancelled/empty agent turn and its preceding user turn.
 * Only removes the pair when the agent produced no steps and no final output —
 * i.e. the run was cancelled before producing anything worth keeping.
 */
export function rollbackEmptyAgentTurn(runId: string): void {
  aiChat.update((s) => {
    const agentIdx = s.messages.findLastIndex(
      (m) => m.agent?.runId === runId && m.agent.snapshot === null
    )
    if (agentIdx === -1) return s
    // Only roll back if there were no tool steps or final answer.
    const msg = s.messages[agentIdx]
    if (msg.agent?.snapshot?.toolLog?.length || msg.agent?.snapshot?.final) return s

    const idsToRemove = new Set([msg.id])
    if (agentIdx > 0 && s.messages[agentIdx - 1].role === 'user') {
      idsToRemove.add(s.messages[agentIdx - 1].id)
    }
    return { ...s, messages: s.messages.filter((m) => !idsToRemove.has(m.id)) }
  })
}

export function setPendingTerminalError(error: TerminalError | null): void {
  aiChat.update((s) => ({ ...s, pendingTerminalError: error }))
}

export function clearPendingTerminalError(): void {
  aiChat.update((s) => ({ ...s, pendingTerminalError: null }))
}

/** Convenience read of the current snapshot. */
export function aiState(): AiChatState {
  return get(aiChat)
}
