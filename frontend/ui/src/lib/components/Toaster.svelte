<script lang="ts">
  import { toasts, dismissToast } from '../stores/toast'
</script>

{#if $toasts.length > 0}
  <div class="toaster" aria-live="polite">
    {#each $toasts as t (t.id)}
      <div class="toast toast-{t.kind}" role="status">
        <span class="toast-msg">{t.message}</span>
        <button type="button" class="toast-close" onclick={() => dismissToast(t.id)} aria-label="Dismiss">×</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toaster {
    position: fixed;
    bottom: 36px; /* above status bar */
    right: 16px;
    z-index: 300;
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background-color: var(--card);
    box-shadow: var(--shadow-md);
    font-size: 13px;
    color: var(--foreground);
    pointer-events: all;
    animation: slide-in 0.15s ease;
    min-width: 220px;
    max-width: 380px;
  }
  .toast-success { border-left: 3px solid var(--primary); }
  .toast-error   { border-left: 3px solid var(--destructive); }
  .toast-info    { border-left: 3px solid var(--muted-foreground); }

  .toast-msg { flex: 1; }

  .toast-close {
    background: none;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    font-size: 16px;
    line-height: 1;
    padding: 0 2px;
  }
  .toast-close:hover { color: var(--foreground); }

  @keyframes slide-in {
    from { opacity: 0; transform: translateX(16px); }
    to   { opacity: 1; transform: translateX(0); }
  }
</style>
