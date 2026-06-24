<script lang="ts">
  import { isDestructiveGate, type PendingToolGate } from '../../stores/agentic'
  import { resolveToolGate } from '../../agentic/agentic-setup'

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
</script>

<div class="cmd-gate">
  {#if reason}<p class="reason">{reason}</p>{/if}
  <pre class="cmd">$ {command}</pre>
  <div class="actions">
    <button type="button" class="gate-btn run" class:danger={destructive} disabled={busy} onclick={run}>
      ▶ Run
    </button>
    <button type="button" class="gate-btn" disabled={busy} onclick={skip}>✗ Skip</button>
  </div>
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
</style>
