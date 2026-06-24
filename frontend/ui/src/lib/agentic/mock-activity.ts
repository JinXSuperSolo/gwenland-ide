import {
  addToolCall,
  addToolResult,
  setAgentBusy,
  setPendingTool,
  setRunId,
  setToolExhausted,
  setToolFinal,
  startToolLoop,
} from '../stores/agentic'
import { aiState, appendAgentTurn, appendUserTurn } from '../stores/ai-chat'
import { createConversation } from '../ai/ai-chat-setup'

/**
 * Dev-only mock driver (M8). Synthesizes a full agent run through the real store
 * mutators — read → search → edit (with an approval pause) → run command → done
 * — so the activity shimmer + timeline + gates can be verified WITHOUT a
 * provider or backend. Exposed as `window.__gwenMockAgent` in dev builds.
 *
 * It drives the same store the live engine writes to, so what you see is exactly
 * the production render path, only the events are local.
 */

const wait = (ms: number) => new Promise<void>((r) => setTimeout(r, ms))

export async function runMockAgentActivity(): Promise<void> {
  // One unified history: the mock run lives in the active conversation.
  if (!aiState().activeConversationId) {
    await createConversation()
    if (!aiState().activeConversationId) return
  }

  const runId = `mock-${crypto.randomUUID()}`
  appendUserTurn('(mock) Refactor Layout.tsx and run the tests')
  startToolLoop()
  setRunId(runId)
  setAgentBusy(true)
  appendAgentTurn(runId)

  // Reading a guessed file.
  addToolCall({ id: 'm1', tool: 'read_file', args: '{"path":"src/components/Layout.tsx"}' })
  await wait(900)
  addToolResult({ id: 'm1', ok: false, content: '', error: 'file not found' })

  // Recover via search.
  addToolCall({ id: 'm2', tool: 'file_search', args: '{"query":"Layout"}' })
  await wait(900)
  addToolResult({ id: 'm2', ok: true, content: 'app/components/Layout.tsx', error: null })

  // Read the real file.
  addToolCall({ id: 'm3', tool: 'read_file', args: '{"path":"app/components/Layout.tsx"}' })
  await wait(800)
  addToolResult({ id: 'm3', ok: true, content: 'export function Layout() { /* ... */ }', error: null })

  // Propose an edit → approval pause.
  addToolCall({
    id: 'm4',
    tool: 'edit_file',
    args: '{"path":"app/components/Layout.tsx","diff":"--- a\\n+++ b\\n-import { Old } from \'./old\'\\n+import { New } from \'./new\'"}',
  })
  setAgentBusy(false)
  setPendingTool({ id: 'm4', tool: 'edit_file', side: 'mutating', risk: null })
  await wait(2200)
  // Auto-approve in the mock (so no Tauri call is needed to advance).
  setPendingTool(null)
  setAgentBusy(true)
  addToolResult({ id: 'm4', ok: true, content: 'Edited app/components/Layout.tsx', error: null })

  // Run validation.
  addToolCall({ id: 'm5', tool: 'run_terminal_cmd', args: '{"command":"npm test","reason":"verify the change"}' })
  await wait(1300)
  addToolResult({ id: 'm5', ok: true, content: 'exit 0\nTests: 12 passed', error: null })

  await wait(400)
  setToolFinal('Updated the Layout import and the test suite passes.')
  setAgentBusy(false)
}

/** A short variant that ends by hitting the iteration cap, for state testing. */
export async function runMockAgentExhausted(): Promise<void> {
  if (!aiState().activeConversationId) {
    await createConversation()
    if (!aiState().activeConversationId) return
  }
  const runId = `mock-${crypto.randomUUID()}`
  appendUserTurn('(mock) Long task')
  startToolLoop()
  setRunId(runId)
  setAgentBusy(true)
  appendAgentTurn(runId)
  addToolCall({ id: 'x1', tool: 'grep_search', args: '{"query":"TODO"}' })
  await wait(900)
  addToolResult({ id: 'x1', ok: true, content: 'src/a.ts:3\nsrc/b.ts:9', error: null })
  setAgentBusy(false)
  setToolExhausted()
}
