<script lang="ts">
  import { isDestructiveGate, type PendingToolGate } from '../../stores/agentic'
  import { resolveToolGate } from '../../agentic/agentic-setup'

  // Inline file-mutation gate (edit/write/delete). Renders the proposed change in
  // the stream with small Accept/Reject controls; once decided it disappears and
  // the tool line collapses to `↳ edit_file ✓`.
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
  const path = $derived(typeof args.path === 'string' ? (args.path as string) : '')
  const destructive = $derived(isDestructiveGate(gate))

  type Line = { text: string; kind: 'add' | 'remove' | 'ctx' }
  const lines = $derived.by<Line[]>(() => {
    if (gate.tool === 'edit_file' && typeof args.diff === 'string') {
      return (args.diff as string).split('\n').map((raw) => {
        if (raw.startsWith('+') && !raw.startsWith('+++')) return { text: raw, kind: 'add' }
        if (raw.startsWith('-') && !raw.startsWith('---')) return { text: raw, kind: 'remove' }
        return { text: raw, kind: 'ctx' }
      })
    }
    if (gate.tool === 'write_file' && typeof args.content === 'string') {
      return (args.content as string).split('\n').map((t) => ({ text: `+${t}`, kind: 'add' as const }))
    }
    return []
  })

  function accept() {
    void resolveToolGate('approve')
  }
  function confirm() {
    if (window.confirm(`Delete ${path || 'this file'}? This cannot be undone automatically.`)) {
      void resolveToolGate('confirm')
    }
  }
  function reject() {
    void resolveToolGate('reject')
  }
</script>

<div class="diff-inline">
  <div class="rule">{gate.tool === 'delete_file' ? 'delete' : ''} {path || '(file)'}</div>

  {#if gate.tool === 'delete_file'}
    <p class="note">The agent wants to delete this file.</p>
  {:else if lines.length > 0}
    <pre class="diff">{#each lines as l}<span class="ln {l.kind}">{l.text}
</span>{/each}</pre>
  {/if}

  <div class="rule end"></div>

  <div class="actions">
    {#if destructive}
      <button type="button" class="gate-btn danger" disabled={busy} onclick={confirm}>✗ Confirm</button>
    {:else}
      <button type="button" class="gate-btn add" disabled={busy} onclick={accept}>✓ Accept</button>
    {/if}
    <button type="button" class="gate-btn" disabled={busy} onclick={reject}>✗ Reject</button>
  </div>
</div>

<style>
  .diff-inline {
    margin: 4px 0 2px 10px;
    font-family: var(--font-mono);
    font-size: 0.75rem;
  }
  .rule {
    color: var(--ai-text-muted);
    opacity: 0.7;
    letter-spacing: 0.04em;
  }
  .rule::before {
    content: '┄┄┄ ';
  }
  .rule::after {
    content: ' ┄┄┄';
  }
  .rule.end {
    height: 0.6em;
  }
  .rule.end::before,
  .rule.end::after {
    content: none;
  }
  .note {
    margin: 4px 0;
    color: var(--ai-text-muted);
    font-family: var(--font-sans);
  }
  .diff {
    margin: 4px 0;
    max-height: 220px;
    overflow: auto;
    white-space: pre;
    line-height: 1.45;
    scrollbar-width: thin;
    scrollbar-color: var(--ai-scrollbar, color-mix(in srgb, var(--primary) 16%, transparent)) transparent;
  }
  .ln {
    display: inline;
    color: var(--ai-text-muted);
  }
  .ln.add {
    color: var(--color-success, #5fb572);
  }
  .ln.remove {
    color: var(--color-error, #e0707c);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
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
  .gate-btn.add {
    color: var(--color-success, #5fb572);
  }
  .gate-btn.danger {
    color: var(--color-error, #e0707c);
  }
  .gate-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
