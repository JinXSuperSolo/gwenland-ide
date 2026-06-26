<script lang="ts">
  import { agentic, isDestructiveGate, type PendingToolGate } from '../../stores/agentic'
  import { resolveToolGate } from '../../agentic/agentic-setup'
  import { agentKillTerminal } from '../../tauri/commands'

  // Inline command-approval gate — `$ command` with Run/Skip. Suspends the stream
  // until decided, then collapses to the tool line.
  let { gate, argsJson, busy = false }: {
    gate: PendingToolGate
    argsJson: string
    busy?: boolean
  } = $props()

  const args = $derived.by(() => {
    try {
      return JSON.parse(argsJson) as Record<string, unknown>
    } catch {
      return {} as Record<string, unknown>
    }
  })
  const command = $derived(typeof args.command === 'string' ? (args.command as string) : '')
  const reason = $derived(typeof args.reason === 'string' ? (args.reason as string) : '')
  const destructive = $derived(isDestructiveGate(gate))

  const isRunning = $derived($agentic.isRunningCommand)
  const outputLines = $derived($agentic.cmdOutputLines)
  const sessionId = $derived($agentic.session?.id ?? '')

  function run() {
    if (destructive) {
      if (!window.confirm(`Run this ${gate.risk ?? 'destructive'} command?\n\n${command}`)) return
      void resolveToolGate('confirm')
    } else {
      void resolveToolGate('approve')
    }
  }
  function skip() {
    void resolveToolGate('reject')
  }
  function kill() {
    if (sessionId) void agentKillTerminal(sessionId)
  }
</script>

<div class="cmd-gate">
  {#if reason}<p class="reason">{reason}</p>{/if}
  <pre class="cmd">$ {command}</pre>

  {#if isRunning}
    <div class="running-banner">
      <span class="pulse-dot"></span>
      <span class="running-label">Agent is executing command…</span>
      <button type="button" class="kill-btn" onclick={kill} title="Kill running command">✕</button>
    </div>
    {#if outputLines.length > 0}
      <div class="cmd-output">
        {#each outputLines as line (line)}
          <div class="output-line">{line}</div>
        {/each}
      </div>
    {/if}
  {:else}
    <div class="actions">
      <button type="button" class="gate-btn run" class:danger={destructive} disabled={busy} onclick={run}>
        ▶ Run
      </button>
      <button type="button" class="gate-btn" disabled={busy} onclick={skip}>✗ Skip</button>
    </div>
  {/if}
</div>

<style>
  .cmd-gate {
    margin: 4px 0 2px 10px;
    font-family: var(--font-mono);
    font-size: 0.75rem;
  }
  .reason {
    margin: 0 0 4px;
    font-family: var(--font-sans);
    color: var(--ai-text-muted);
  }
  .cmd {
    margin: 0 0 4px;
    padding: 6px 8px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--ai-bg-surface) 80%, transparent);
    color: var(--ai-text-primary);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .gate-btn {
    padding: 2px 10px;
    border-radius: 4px;
    font-size: 0.75rem;
    border: 1px solid currentColor;
    background: transparent;
    color: var(--ai-text-muted);
    cursor: pointer;
  }
  .gate-btn:hover:not(:disabled) {
    color: var(--ai-text-primary);
  }
  .gate-btn.run {
    color: var(--color-success, #5fb572);
  }
  .gate-btn.run.danger {
    color: var(--color-error, #e0707c);
  }
  .gate-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  /* Running state */
  .running-banner {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 0;
    color: var(--color-success, #5fb572);
    font-family: var(--font-sans);
  }
  .pulse-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
    flex-shrink: 0;
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.35; transform: scale(0.75); }
  }
  .running-label {
    flex: 1;
    font-size: 0.75rem;
  }
  .kill-btn {
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 0.7rem;
    border: 1px solid var(--color-error, #e0707c);
    background: transparent;
    color: var(--color-error, #e0707c);
    cursor: pointer;
    line-height: 1.4;
  }
  .kill-btn:hover {
    background: color-mix(in srgb, var(--color-error, #e0707c) 15%, transparent);
  }

  /* Live output scroll box */
  .cmd-output {
    margin-top: 4px;
    max-height: 160px;
    overflow-y: auto;
    padding: 4px 6px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--ai-bg-surface) 60%, transparent);
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--ai-text-muted);
  }
  .output-line {
    white-space: pre-wrap;
    word-break: break-all;
    line-height: 1.4;
  }
</style>
