import { writable } from 'svelte/store'

/**
 * The unified assistant's interaction mode. One chat pane, one history; the mode
 * only changes behaviour, never resets the conversation.
 *
 *  - `ask`   — read-only Q&A about the code (no edits).
 *  - `edit`  — ask for a single inline diff you accept/reject in the stream.
 *  - `agent` — the human-gated ReAct tool loop (reads freely; edits & commands
 *              stop inline for approval).
 *
 * The last-used mode is persisted so the pane reopens where you left off.
 */
export type AssistantMode = 'ask' | 'edit' | 'agent'

export interface ModeMeta {
  label: string
  /** Short one-liner shown in the picker + as the empty-state headline. */
  hint: string
  /** Icon name from the Icon.svelte registry. */
  icon: string
}

export const MODE_META: Record<AssistantMode, ModeMeta> = {
  ask: { label: 'Ask', hint: 'Ask anything about your code', icon: 'chat-bubble' },
  edit: { label: 'Edit', hint: 'Propose an edit you can accept inline', icon: 'edit-pencil' },
  agent: { label: 'Agent', hint: 'Act, edit & run — with approval', icon: 'sparks' },
}

export const ASSISTANT_MODES: AssistantMode[] = ['ask', 'edit', 'agent']

const STORAGE_KEY = 'gwen.assistantMode'

/** Map legacy persisted modes (chat/explain/plan) onto the current set. */
function normalizeMode(v: string | null): AssistantMode | null {
  switch (v) {
    case 'ask':
    case 'edit':
    case 'agent':
      return v
    case 'chat':
    case 'explain':
    case 'plan':
      return 'ask'
    default:
      return null
  }
}

function loadMode(): AssistantMode {
  try {
    return normalizeMode(localStorage.getItem(STORAGE_KEY)) ?? 'ask'
  } catch {
    return 'ask'
  }
}

export const assistantMode = writable<AssistantMode>(loadMode())

assistantMode.subscribe((mode) => {
  try {
    localStorage.setItem(STORAGE_KEY, mode)
  } catch {
    /* storage unavailable */
  }
})
