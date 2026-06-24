<script lang="ts">
  import { promptDialog, confirmPrompt, cancelPrompt } from '../stores/prompt-dialog'

  let value = $state('')
  let inputEl = $state<HTMLInputElement>()

  // Seed the field and focus+select it whenever the dialog opens.
  $effect(() => {
    if ($promptDialog.open) {
      value = $promptDialog.value
      queueMicrotask(() => {
        inputEl?.focus()
        inputEl?.select()
      })
    }
  })

  function submit() {
    const v = value.trim()
    if (!v) return
    confirmPrompt(v)
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      submit()
    } else if (e.key === 'Escape') {
      e.preventDefault()
      cancelPrompt()
    }
  }
</script>

{#if $promptDialog.open}
  <div
    class="prompt-overlay gw-anim-overlay"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) cancelPrompt()
    }}
  >
    <div class="prompt gw-anim-pop" role="dialog" aria-modal="true" aria-label={$promptDialog.title}>
      <div class="prompt-title">{$promptDialog.title}</div>
      {#if $promptDialog.label}
        <label class="prompt-label" for="prompt-input">{$promptDialog.label}</label>
      {/if}
      <input
        id="prompt-input"
        class="prompt-input"
        bind:this={inputEl}
        bind:value
        placeholder={$promptDialog.placeholder}
        onkeydown={onKeydown}
      />
      <div class="prompt-actions">
        <button type="button" class="prompt-btn" onclick={cancelPrompt}>Cancel</button>
        <button type="button" class="prompt-btn primary" disabled={!value.trim()} onclick={submit}>
          {$promptDialog.confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .prompt-overlay {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 18vh;
    background-color: rgba(0, 0, 0, 0.4);
  }
  .prompt {
    width: 420px;
    max-width: 90vw;
    background-color: var(--popover);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-xl);
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .prompt-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--foreground);
  }
  .prompt-label {
    font-size: 12px;
    color: var(--muted-foreground);
  }
  .prompt-input {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 11px;
    background-color: var(--input);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    outline: none;
  }
  .prompt-input:focus {
    border-color: var(--primary);
  }
  .prompt-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  .prompt-btn {
    padding: 7px 14px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background-color: var(--secondary);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    cursor: pointer;
    transition: background-color 0.12s ease, border-color 0.12s ease;
  }
  .prompt-btn:hover {
    background-color: var(--sidebar-accent);
  }
  .prompt-btn.primary {
    background-color: var(--primary);
    color: var(--primary-foreground);
    border-color: var(--primary);
  }
  .prompt-btn.primary:hover:not(:disabled) {
    filter: brightness(1.05);
  }
  .prompt-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
