<script lang="ts">
  import { confirmDialog, acceptConfirm, cancelConfirm } from '../stores/confirm-dialog'

  function onKeydown(e: KeyboardEvent) {
    if (!$confirmDialog.open) return
    if (e.key === 'Enter') { e.preventDefault(); acceptConfirm() }
    else if (e.key === 'Escape') { e.preventDefault(); cancelConfirm() }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if $confirmDialog.open}
  <div
    class="overlay gw-anim-overlay"
    role="presentation"
    onclick={(e) => { if (e.target === e.currentTarget) cancelConfirm() }}
  >
    <div class="dialog gw-anim-pop" role="alertdialog" aria-modal="true" aria-label={$confirmDialog.title}>
      <div class="dialog-title">{$confirmDialog.title}</div>
      <div class="dialog-message">{$confirmDialog.message}</div>
      <div class="dialog-actions">
        <button type="button" class="btn" onclick={cancelConfirm}>Cancel</button>
        <button
          type="button"
          class="btn confirm"
          class:danger={$confirmDialog.danger}
          onclick={acceptConfirm}
        >
          {$confirmDialog.confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    justify-content: center;
    align-items: center;
    background-color: rgba(0, 0, 0, 0.4);
  }
  .dialog {
    width: 380px;
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
  .dialog-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--foreground);
  }
  .dialog-message {
    font-size: 13px;
    color: var(--muted-foreground);
    line-height: 1.5;
  }
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  .btn {
    padding: 7px 14px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background-color: var(--secondary);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    cursor: pointer;
    transition: background-color 0.12s ease;
  }
  .btn:hover {
    background-color: var(--sidebar-accent);
  }
  .btn.confirm {
    background-color: var(--primary);
    color: var(--primary-foreground);
    border-color: var(--primary);
  }
  .btn.confirm:hover {
    filter: brightness(1.05);
  }
  .btn.confirm.danger {
    background-color: var(--destructive);
    color: var(--destructive-foreground);
    border-color: var(--destructive);
  }
</style>
