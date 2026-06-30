import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  aiChat,
  appendAgentTurn,
  appendUserTurn,
  rollbackEmptyAgentTurn,
  setStreamError,
  setUnsentInput,
  type AiChatState,
  type ChatMessage,
} from './ai-chat'
import { runSlashCommand } from '../ai/slash-command-setup'

const baseState = (): AiChatState => ({
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
})

function streamingAssistant(id: string, content = ''): ChatMessage {
  return {
    id,
    role: 'assistant',
    content,
    streaming: true,
    timestamp: '2026-06-29T00:00:00.000Z',
  }
}

beforeEach(() => {
  let id = 0
  vi.stubGlobal('crypto', {
    randomUUID: () => `msg-${++id}`,
  })
  aiChat.set(baseState())
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('aiChat history mutations', () => {
  it('appends user turns and /clear removes visible history and errors', async () => {
    aiChat.update((s) => ({
      ...s,
      lastError: { kind: 'storage_error', data: 'old failure' },
    }))
    setUnsentInput('draft prompt')

    const id = appendUserTurn('hello Gwen')
    expect(id).toBe('msg-1')
    expect(get(aiChat).messages.map((m) => [m.id, m.role, m.content])).toEqual([
      ['msg-1', 'user', 'hello Gwen'],
    ])
    expect(get(aiChat).unsentInput).toBe('')

    await expect(runSlashCommand('clear')).resolves.toEqual({ setInput: '' })
    expect(get(aiChat).messages).toEqual([])
    expect(get(aiChat).lastError).toBeNull()
  })

  it('cancelled empty streams roll back the assistant and preceding user turn', () => {
    const userId = appendUserTurn('draft request')
    aiChat.update((s) => ({
      ...s,
      activeStreamId: 'stream-1',
      messages: [...s.messages, streamingAssistant('assistant-1')],
    }))

    setStreamError('stream-1', { kind: 'cancelled' })

    const state = get(aiChat)
    expect(state.activeStreamId).toBeNull()
    expect(state.lastError).toBeNull()
    expect(state.messages.find((m) => m.id === userId)).toBeUndefined()
    expect(state.messages.find((m) => m.id === 'assistant-1')).toBeUndefined()
  })

  it('cancelled partial streams keep received history without showing an error', () => {
    appendUserTurn('draft request')
    aiChat.update((s) => ({
      ...s,
      activeStreamId: 'stream-1',
      messages: [...s.messages, streamingAssistant('assistant-1', 'partial answer')],
    }))

    setStreamError('stream-1', { kind: 'cancelled' })

    const state = get(aiChat)
    expect(state.activeStreamId).toBeNull()
    expect(state.lastError).toBeNull()
    expect(state.messages.map((m) => [m.role, m.content, m.streaming])).toEqual([
      ['user', 'draft request', false],
      ['assistant', 'partial answer', false],
    ])
  })

  it('rolls back empty agent turns with their user request', () => {
    appendUserTurn('inspect this')
    const agentMessageId = appendAgentTurn('run-1')

    rollbackEmptyAgentTurn('run-1')

    const state = get(aiChat)
    expect(state.messages.find((m) => m.id === agentMessageId)).toBeUndefined()
    expect(state.messages).toEqual([])
  })
})
