import { get } from 'svelte/store'
import type { UnlistenFn } from '@tauri-apps/api/event'

import * as cmd from '../tauri/commands'
import type { ContextAttachment, ImageAttachment } from '../tauri/commands'
import { workspace } from '../stores/workspace'
import { tabs, isEditorTab } from '../stores/tabs'
import { activeSelection } from '../editor/active-editor'
import {
  aiChat,
  appendToken,
  appendThinking,
  finaliseStream,
  setStreamError,
  setMessageDiff,
  type ChatMessage,
} from '../stores/ai-chat'
import { createThinkParser, splitThinking, type ThinkParser } from './thinking-parser'
import { detectDiff } from './diff-detection'
import { activeSystemPrefix } from './persona-setup'

/**
 * AI chat orchestration (Milestone 4). Bridges the `ai-chat` store and the
 * engine-backed Tauri commands. The store stays Tauri-free; all `invoke`/event
 * wiring lives here. Critically, streaming listeners are registered BEFORE
 * `ai_send` is invoked so no chunk can be missed (Requirement 10.10).
 */

let unlistenChunk: UnlistenFn | null = null
let unlistenDone: UnlistenFn | null = null
let unlistenError: UnlistenFn | null = null
/** Per-stream `<think>` parser; recreated for each send. */
let thinkParser: ThinkParser | null = null

function teardownListeners(): void {
  unlistenChunk?.()
  unlistenChunk = null
  unlistenDone?.()
  unlistenDone = null
  unlistenError?.()
  unlistenError = null
  thinkParser = null
}

/** Open project root, or null when no folder is open. */
export function projectRoot(): string | null {
  return get(workspace).folderPath
}

// --- Attachment builders (Requirement 14.2) --------------------------------

/** Active editor tab's on-disk path, or null (untitled/none). */
export function currentFilePath(): string | null {
  const s = get(tabs)
  const t = s.tabs.find((tab) => tab.id === s.activeId)
  return t && isEditorTab(t) && t.path ? t.path : null
}

/** Attach the current file (only when it has a saved path). */
export function currentFileAttachment(): ContextAttachment | null {
  const path = currentFilePath()
  return path ? { type: 'file', path } : null
}

/** Attach the current selection (needs a saved path + non-empty selection). */
export function currentSelectionAttachment(): ContextAttachment | null {
  const path = currentFilePath()
  const text = activeSelection()
  if (!path || !text) return null
  return { type: 'selection', path, content: text }
}

// --- Initialization / loading ----------------------------------------------

/** Seed provider/model from engine settings, then load key status + models. */
export async function initAiChat(): Promise<void> {
  try {
    const settings = await cmd.loadEngineSettings()
    aiChat.update((s) => ({
      ...s,
      activeProvider: settings.ai.active_provider || s.activeProvider,
      activeModel: settings.ai.active_model || s.activeModel,
      genericProviders: settings.ai.generic_providers ?? [],
    }))
  } catch {
    /* fall back to store defaults */
  }
  await Promise.all([loadConversations(), refreshKeyStatus(), loadModels()])
}

/** Refresh the conversation list (newest first). */
export async function loadConversations(): Promise<void> {
  try {
    const conversations = await cmd.conversationList()
    aiChat.update((s) => ({ ...s, conversations }))
  } catch {
    /* leave existing list */
  }
}

/** Whether a key is stored for the active provider. */
export async function refreshKeyStatus(): Promise<void> {
  const provider = get(aiChat).activeProvider
  try {
    const hasKey = await cmd.aiCheckKey(provider)
    aiChat.update((s) => ({ ...s, hasKey }))
  } catch {
    aiChat.update((s) => ({ ...s, hasKey: false }))
  }
}

/** Load the model list for the active provider; default the model if unset. */
export async function loadModels(): Promise<void> {
  const provider = get(aiChat).activeProvider
  try {
    const models = await cmd.aiListModels(provider)
    aiChat.update((s) => {
      let activeModel = s.activeModel
      if ((!activeModel || !models?.some((m) => m.id === activeModel)) && models?.length) {
        activeModel = models[0].id
      }
      return { ...s, models, activeModel }
    })
  } catch {
    aiChat.update((s) => ({ ...s, models: null }))
  }
}

/** Switch provider: persist, then refresh key status + models. */
export async function setProvider(provider: string): Promise<void> {
  aiChat.update((s) => ({ ...s, activeProvider: provider, activeModel: '', models: null }))
  await persistActive()
  await Promise.all([refreshKeyStatus(), loadModels()])
}

/** Switch model: persist the preference. */
export async function setModel(model: string): Promise<void> {
  aiChat.update((s) => ({ ...s, activeModel: model }))
  await persistActive()
}

/** Persist the active provider/model to engine settings (best-effort). */
async function persistActive(): Promise<void> {
  try {
    const settings = await cmd.loadEngineSettings()
    const { activeProvider, activeModel } = get(aiChat)
    settings.ai.active_provider = activeProvider
    settings.ai.active_model = activeModel
    await cmd.saveEngineSettings(settings)
  } catch {
    /* non-fatal */
  }
}

// --- Conversation management -----------------------------------------------

/** Create a new conversation in the open project and select it. */
export async function createConversation(): Promise<void> {
  const root = projectRoot()
  if (!root) return
  const { activeProvider, activeModel } = get(aiChat)
  try {
    const meta = await cmd.conversationNew(root, 'New Conversation', activeProvider, activeModel)
    aiChat.update((s) => ({
      ...s,
      conversations: [meta, ...s.conversations],
      activeConversationId: meta.id,
      messages: [],
      lastError: null,
    }))
  } catch (e) {
    aiChat.update((s) => ({ ...s, lastError: { kind: 'storage_error', data: String(e) } }))
  }
}

/** Select a conversation and load its turns into messages. */
export async function selectConversation(id: string): Promise<void> {
  aiChat.update((s) => ({ ...s, activeConversationId: id, messages: [], lastError: null }))
  try {
    const turns = await cmd.conversationLoad(id)
    const messages: ChatMessage[] = []
    for (const turn of turns) {
      for (const m of turn.messages) {
        if (m.role === 'user') {
          messages.push({
            id: crypto.randomUUID(),
            role: m.role,
            content: m.content,
            streaming: false,
            timestamp: turn.timestamp,
          })
        } else if (m.role === 'assistant') {
          // Reconstruct the thinking trace from any persisted `<think>` markers
          // so raw tags never render; old turns without markers load as-is.
          const { thinking, answer } = splitThinking(m.content)
          messages.push({
            id: crypto.randomUUID(),
            role: m.role,
            content: answer,
            streaming: false,
            timestamp: turn.timestamp,
            thinking: thinking
              ? { content: thinking, streaming: false, startedAt: null, endedAt: null, collapsed: true }
              : undefined,
          })
        }
      }
    }
    aiChat.update((s) => (s.activeConversationId === id ? { ...s, messages } : s))
  } catch (e) {
    aiChat.update((s) => ({ ...s, lastError: { kind: 'storage_error', data: String(e) } }))
  }
}

export async function renameConversation(id: string, title: string): Promise<void> {
  await cmd.conversationRename(id, title)
  await loadConversations()
}

/** A conversation still carrying its default title (never user-renamed). */
function hasDefaultTitle(id: string): boolean {
  const meta = get(aiChat).conversations.find((c) => c.id === id)
  return !meta || meta.title === 'New Conversation' || meta.title.trim() === ''
}

/** Clean a model-proposed title: strip quotes/trailing punctuation, cap length. */
function cleanTitle(raw: string): string {
  let t = raw.split('\n')[0].trim()
  t = t.replace(/^["'`]+|["'`]+$/g, '').replace(/[.!?]+$/, '').trim()
  if (t.length > 48) t = t.slice(0, 48).trim() + '…'
  return t
}

/**
 * GWEN-324: after the first AI response, auto-name an as-yet-unnamed
 * conversation via a short side-prompt to the model. Best-effort — any failure
 * leaves the default title untouched (no banner). Skipped if the user already
 * renamed it.
 */
async function autoNameConversation(id: string, firstUserMessage: string): Promise<void> {
  if (!hasDefaultTitle(id)) return
  const msg = firstUserMessage.trim()
  if (!msg) return
  const { activeProvider, activeModel } = get(aiChat)
  try {
    const raw = await cmd.aiComplete(
      `Give a 3-5 word title for this conversation (no quotes, no punctuation): ${msg}`,
      activeProvider,
      activeModel
    )
    const title = cleanTitle(raw)
    // Re-check: the user may have renamed it while the side-prompt was in flight.
    if (title && hasDefaultTitle(id)) await renameConversation(id, title)
  } catch {
    /* keep default title */
  }
}

export async function deleteConversation(id: string): Promise<void> {
  await cmd.conversationDelete(id)
  aiChat.update((s) => {
    const conversations = s.conversations.filter((c) => c.id !== id)
    const wasActive = s.activeConversationId === id
    return {
      ...s,
      conversations,
      activeConversationId: wasActive ? null : s.activeConversationId,
      messages: wasActive ? [] : s.messages,
    }
  })
}

export async function setTrainingOptIn(id: string, optIn: boolean): Promise<void> {
  await cmd.conversationSetTrainingOptIn(id, optIn)
  aiChat.update((s) => ({
    ...s,
    conversations: s.conversations.map((c) =>
      c.id === id ? { ...c, training_opt_in: optIn } : c
    ),
  }))
}

// --- Diff detection --------------------------------------------------------

/**
 * Inspect a finished assistant message for a unified-diff proposal and record
 * the result on the message. Non-blocking and non-destructive: the raw answer
 * text stays visible whether parsing succeeds, fails, or finds nothing.
 */
async function runDiffDetection(messageId: string): Promise<void> {
  const msg = get(aiChat).messages.find((m) => m.id === messageId)
  if (!msg) return
  const result = await detectDiff(msg.content)
  if (result.kind === 'proposal') {
    setMessageDiff(messageId, {
      diff: { files: result.files, fileCount: result.fileCount, hunkCount: result.hunkCount },
    })
  } else if (result.kind === 'failed') {
    setMessageDiff(messageId, { diffError: result.message })
  }
}

// --- Streaming -------------------------------------------------------------

/**
 * Send a message. Inserts the user message + an empty streaming assistant
 * message, registers chunk/done/error listeners, then invokes `ai_send`.
 * No-op if there is no active conversation or a stream is already in flight.
 */
export async function sendMessage(
  message: string,
  attachments: ContextAttachment[] = [],
  images: ImageAttachment[] = []
): Promise<void> {
  let state = get(aiChat)
  if (state.activeStreamId) return

  const trimmed = message.trim()
  // Allow an image-only turn (no text) for multimodal models.
  if (!trimmed && images.length === 0) return

  // GWEN-324: the empty-state CTA is gone, so the composer is always live. If no
  // conversation exists yet (and a project is open), create one on first send.
  if (!state.activeConversationId) {
    await createConversation()
    state = get(aiChat)
    if (!state.activeConversationId) return // no project open / create failed
  }

  // Whether this is the first user turn — drives post-response auto-naming.
  const isFirstTurn = state.messages.length === 0
  const conversationId = state.activeConversationId

  const userMsgId = crypto.randomUUID()
  const asstMsgId = crypto.randomUUID()
  const now = new Date().toISOString()
  aiChat.update((s) => ({
    ...s,
    lastError: null,
    unsentInput: '',
    messages: [
      ...s.messages,
      {
        id: userMsgId,
        role: 'user',
        content: trimmed,
        streaming: false,
        timestamp: now,
        attachments,
        images: images.length ? images : undefined,
      },
      { id: asstMsgId, role: 'assistant', content: '', streaming: true, timestamp: now },
    ],
  }))

  // The UI owns the stream id so listeners exist before Rust can emit.
  const streamId = crypto.randomUUID()
  aiChat.update((s) => ({ ...s, activeStreamId: streamId }))

  teardownListeners()
  thinkParser = createThinkParser()
  unlistenChunk = await cmd.onAiChunk(streamId, (text) => {
    // Split `<think>` reasoning from the answer before it reaches the store
    // (chunk-safe across tag boundaries — Requirement 7.2/7.6).
    const { thinking, answer } = (thinkParser ?? createThinkParser()).feed(text)
    if (thinking) appendThinking(streamId, thinking)
    if (answer) appendToken(streamId, answer)
  })
  unlistenDone = await cmd.onAiDone(streamId, () => {
    const tail = thinkParser?.flush()
    if (tail?.thinking) appendThinking(streamId, tail.thinking)
    if (tail?.answer) appendToken(streamId, tail.answer)
    finaliseStream(streamId)
    teardownListeners()
    // Refresh the manifest so updated_at ordering reflects this turn, then
    // auto-name the conversation from the first turn (GWEN-324).
    loadConversations().then(() => {
      if (isFirstTurn) void autoNameConversation(conversationId, trimmed)
    })
    // Detect a reviewable diff in the final answer (non-blocking; Req 10.4-10.7).
    void runDiffDetection(asstMsgId)
  })
  unlistenError = await cmd.onAiError(streamId, (error) => {
    setStreamError(streamId, error)
    teardownListeners()
  })

  try {
    await cmd.aiSend({
      streamId,
      conversationId,
      message: trimmed,
      attachments,
      images,
      provider: state.activeProvider,
      model: state.activeModel,
      // GWEN-334: per-workspace persona/system prompt, layered over the base.
      systemPrefix: activeSystemPrefix(),
    })
  } catch (e) {
    // Failure before the stream task started (e.g. duplicate id, bad provider).
    // Remove the placeholder assistant message and surface the error.
    aiChat.update((s) => ({
      ...s,
      messages: s.messages.filter((m) => m.id !== asstMsgId),
      activeStreamId: null,
      lastError: { kind: 'provider_error', data: String(e) },
    }))
    teardownListeners()
  }
}

/**
 * GWEN-326: edit a user message and roll the conversation back to that point,
 * then re-submit the new text. Every message at/after the edited one is dropped
 * (in the UI) and the JSONL is truncated to the turns that precede it, so history
 * stays consistent. No-op while a stream is active.
 *
 * A "turn" in the JSONL is one user+assistant exchange, so the number of turns to
 * keep equals the count of completed assistant messages before the edited one.
 */
export async function editAndResubmit(messageId: string, newText: string): Promise<void> {
  const state = get(aiChat)
  if (state.activeStreamId) return
  const idx = state.messages.findIndex((m) => m.id === messageId)
  if (idx < 0) return
  const conversationId = state.activeConversationId
  const trimmed = newText.trim()
  if (!trimmed) return

  // Carry over any attachments/images the original turn had.
  const original = state.messages[idx]
  const attachments = original.attachments ?? []
  const images = original.images ?? []

  // Completed assistant turns strictly before the edited message → keep_count.
  const keepCount = state.messages
    .slice(0, idx)
    .filter((m) => m.role === 'assistant' && !m.streaming).length

  // Drop the edited message and everything after it from the UI.
  aiChat.update((s) => ({
    ...s,
    messages: s.messages.slice(0, idx),
    lastError: null,
  }))

  // Truncate the persisted history to match (best-effort; UI rollback already
  // happened, so a storage hiccup only desyncs the file, not the screen).
  if (conversationId) {
    try {
      await cmd.conversationTruncate(conversationId, keepCount)
    } catch {
      /* non-fatal */
    }
  }

  await sendMessage(trimmed, attachments, images)
}

/** Cancel the in-flight stream (keeps partial text, no red banner). */
export async function cancelStream(): Promise<void> {
  const { activeStreamId } = get(aiChat)
  if (!activeStreamId) return
  try {
    await cmd.aiCancel(activeStreamId)
  } catch {
    /* cancelling a finished stream is a no-op */
  }
}
