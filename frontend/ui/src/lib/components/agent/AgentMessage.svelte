<script lang="ts">
  import { agentic, clearAgentError } from '../../stores/agentic'
  import type { ChatMessage } from '../../stores/ai-chat'
  import { aiErrorMessage } from '../../tauri/commands'
  import { currentActivityLabel, deriveActivities, runState } from '../../agent-activity'
  import ActivityRow from '../chat/ActivityRow.svelte'
  import ThinkingDisclosure from '../chat/ThinkingDisclosure.svelte'
  import DiffInline from '../chat/DiffInline.svelte'
  import CommandGate from '../chat/CommandGate.svelte'
  import AskUserInline from '../chat/AskUserInline.svelte'

  // One agent run in the unified chat stream (M8). A compact active row shimmers
  // while working ("Reading…", "Running…", "Waiting for approval…"); the tool
  // timeline lives under it, collapsed. Renders live from the `agentic` engine
  // while active, else from the run's frozen snapshot.
  let { message }: { message: ChatMessage } = $props()

  const live = $derived(
    !!message.agent && message.agent.snapshot === null && $agentic.runId === message.agent.runId
  )
  const snap = $derived(message.agent?.snapshot ?? null)

  const toolLog = $derived(live ? $agentic.toolLog : (snap?.toolLog ?? []))
  const pendingTool = $derived(live ? $agentic.pendingTool : null)
  const pendingAsk = $derived(live ? $agentic.pendingAsk : null)
  const busy = $derived(live && $agentic.busy)
  const gateArgs = $derived(
    pendingTool ? (toolLog.find((e) => e.id === pendingTool.id)?.args ?? '{}') : '{}'
  )

  const activities = $derived(deriveActivities(toolLog))
  const working = $derived(busy && !pendingTool && !pendingAsk)
  const currentLabel = $derived(
    currentActivityLabel({ toolLog, pendingTool, pendingAsk, busy })
  )
  const stepCount = $derived(activities.length)
  const summary = $derived(`Thought · ${stepCount} step${stepCount === 1 ? '' : 's'}`)
  const headerLabel = $derived(currentLabel ?? summary)

  const final = $derived(live ? $agentic.toolFinal : (snap?.final ?? null))
  const stopped = $derived(live ? $agentic.toolStopped : (snap?.stopped ?? null))
  const error = $derived(live ? $agentic.lastError : null)
  const cancelled = $derived(live && $agentic.phase === 'cancelled')
  const state = $derived(
    runState({
      active: live && $agentic.toolActive,
      busy,
      stopped,
      hasError: !!error,
      cancelled,
    })
  )
</script>

<div class="agent-msg">
  {#if stepCount > 0 || working || pendingTool || pendingAsk}
    <ThinkingDisclosure label={headerLabel} shimmer={working}>
      {#each activities as activity (activity.id)}
        <ActivityRow {activity} />
      {/each}
    </ThinkingDisclosure>
  {/if}

  <!-- A pending gate always surfaces (outside the collapsed timeline). -->
  {#if pendingTool}
    {#if pendingTool.side === 'terminal'}
      <CommandGate gate={pendingTool} argsJson={gateArgs} {busy} />
    {:else}
      <DiffInline gate={pendingTool} argsJson={gateArgs} {busy} />
    {/if}
  {/if}
  {#if pendingAsk}
    <AskUserInline ask={pendingAsk} {busy} />
  {/if}

  {#if state === 'completed' && final}
    <div class="msg"><span class="dot"></span>{final}</div>
  {:else if stopped === 'exhausted'}
    <div class="msg muted"><span class="dot"></span>Stopped at the step limit — send another message to continue.</div>
  {:else if state === 'cancelled'}
    <div class="msg muted"><span class="dot"></span>Cancelled.</div>
  {/if}

  {#if error}
    <div class="err" role="alert">
      <span>{aiErrorMessage(error)}</span>
      <button type="button" class="err-x" aria-label="Dismiss" onclick={clearAgentError}>✕</button>
    </div>
  {/if}
</div>

<style>
  .agent-msg {
    display: flex;
    flex-direction: column;
    min-width: 0;
    max-width: 100%;
  }
  .msg {
    margin: 4px 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--ai-text-primary);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .msg.muted {
    color: var(--ai-text-muted);
  }
  .dot {
    display: inline-block;
    width: 5px;
    height: 5px;
    margin-right: 7px;
    border-radius: 50%;
    background: var(--ai-primary);
    vertical-align: middle;
  }
  .err {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 6px 0;
    font-size: 12px;
    color: #ffb4a9;
  }
  .err-x {
    border: 0;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
</style>
