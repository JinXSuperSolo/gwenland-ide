<script lang="ts">
  import type { PendingAsk } from '../../stores/agentic'
  import { answerToolAsk } from '../../agentic/agentic-setup'

  // Inline A/B/C/D choice. Single-select resolves on click; multi accumulates then
  // submits. After a choice the gate clears and the tool line collapses.
  let { ask, busy = false }: { ask: PendingAsk; busy?: boolean } = $props()

  const letters = 'ABCDEFGH'
  let chosen = $state<string[]>([])

  function pick(option: string) {
    if (busy) return
    void answerToolAsk([option])
  }
  function toggle(option: string) {
    chosen = chosen.includes(option) ? chosen.filter((o) => o !== option) : [...chosen, option]
  }
  function submit() {
    if (busy || chosen.length === 0) return
    void answerToolAsk(chosen)
  }
</script>

<div class="ask-inline">
  {#if ask.prompt}<p class="prompt">{ask.prompt}</p>{/if}
  <div class="choices">
    {#each ask.options as option, i}
      {#if ask.multi}
        <button type="button" class="choice" class:on={chosen.includes(option)} disabled={busy} onclick={() => toggle(option)}>
          <span class="key">{chosen.includes(option) ? '✓' : (letters[i] ?? '•')}</span>{option}
        </button>
      {:else}
        <button type="button" class="choice" disabled={busy} onclick={() => pick(option)}>
          <span class="key">{letters[i] ?? '•'}</span>{option}
        </button>
      {/if}
    {/each}
  </div>
  {#if ask.multi}
    <button type="button" class="submit" disabled={busy || chosen.length === 0} onclick={submit}>
      Submit{chosen.length ? ` (${chosen.length})` : ''}
    </button>
  {/if}
</div>

<style>
  .ask-inline {
    margin: 4px 0 2px 10px;
  }
  .prompt {
    margin: 0 0 6px;
    color: var(--ai-text-primary);
    font-size: 13px;
    line-height: 1.45;
  }
  .choices {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .choice {
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: left;
    padding: 5px 9px;
    border-radius: 4px;
    border: 1px solid currentColor;
    background: transparent;
    color: var(--ai-text-muted);
    font-size: 0.8rem;
    cursor: pointer;
  }
  .choice:hover:not(:disabled) {
    color: var(--ai-text-primary);
  }
  .choice.on {
    color: var(--ai-primary-light);
  }
  .choice:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .key {
    flex-shrink: 0;
    width: 16px;
    text-align: center;
    font-family: var(--font-mono);
    font-weight: 700;
    opacity: 0.8;
  }
  .submit {
    margin-top: 6px;
    padding: 3px 12px;
    border-radius: 4px;
    border: 1px solid currentColor;
    background: transparent;
    color: var(--ai-primary-light);
    font-size: 0.75rem;
    cursor: pointer;
  }
  .submit:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
