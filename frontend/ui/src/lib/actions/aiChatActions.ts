/**
 * AI chat panel context actions. Right-clicking the AI panel opens these via the
 * same shared registry as every other surface (scope `ai_chat`). Copy works on
 * the live text selection; Copy Conversation grabs the whole transcript.
 */
import { get } from 'svelte/store'
import { registry } from '../context-menu/actionRegistry'
import type { ContextAction } from '../context-menu/contextTypes'
import { aiChat } from '../stores/ai-chat'
import { workspace } from '../stores/workspace'
import { cancelStream, createConversation } from '../ai/ai-chat-setup'
import { openSettings } from '../stores/ui'

const copyText = (text: string) => void navigator.clipboard.writeText(text).catch(() => {})

const aiChatActions: ContextAction[] = [
  {
    id: 'ai.copy',
    label: 'Copy',
    icon: 'copy',
    group: 'clipboard',
    order: 10,
    shortcut: 'Ctrl+C',
    when: (ctx) => ctx.scope === 'ai_chat',
    enabled: (ctx) => !!ctx.selectionText,
    run: (ctx) => {
      if (ctx.selectionText) copyText(ctx.selectionText)
    },
  },
  {
    id: 'ai.copyConversation',
    label: 'Copy Conversation',
    icon: 'list',
    group: 'clipboard',
    order: 20,
    when: (ctx) => ctx.scope === 'ai_chat',
    enabled: () => get(aiChat).messages.length > 0,
    run: () => {
      const text = get(aiChat)
        .messages.map((m) => `${m.role === 'user' ? 'You' : 'Assistant'}: ${m.content}`)
        .join('\n\n')
      if (text) copyText(text)
    },
  },
  {
    id: 'ai.newConversation',
    label: 'New Conversation',
    icon: 'plus',
    group: 'session',
    order: 30,
    when: (ctx) => ctx.scope === 'ai_chat',
    enabled: () => !!get(workspace).folderPath && get(aiChat).activeStreamId === null,
    run: () => void createConversation(),
  },
  {
    id: 'ai.stop',
    label: 'Stop Generating',
    icon: 'xmark',
    group: 'session',
    order: 40,
    when: (ctx) => ctx.scope === 'ai_chat',
    enabled: () => get(aiChat).activeStreamId !== null,
    run: () => void cancelStream(),
  },
  {
    id: 'ai.settings',
    label: 'Settings',
    icon: 'settings',
    group: 'view',
    order: 50,
    when: (ctx) => ctx.scope === 'ai_chat',
    run: () => openSettings(),
  },
]

/** Register the AI chat action set into the shared registry (called at init). */
export function registerAiChatActions(): void {
  registry.registerAll(aiChatActions)
}
