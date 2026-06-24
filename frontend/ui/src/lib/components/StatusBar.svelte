<script lang="ts">
  import { cursor } from '../stores/cursor'
  import { aiChat, toggleAiChat } from '../stores/ai-chat'
  import Icon from './Icon.svelte'
  import LspStatusIndicator from './LspStatusIndicator.svelte'
  import GitStatusBar from './GitStatusBar.svelte'
</script>

<footer class="status-bar" aria-label="Status Bar">
  <div class="status-left">
    <GitStatusBar />
    {#if $cursor}
      <span class="status-item">Ln {$cursor.line}, Col {$cursor.col}</span>
      <span class="status-item">UTF-8</span>
    {/if}
  </div>
  <div class="status-right">
    <LspStatusIndicator />
    <button
      class="ai-btn"
      class:active={$aiChat.isOpen}
      title="AI Chat (coming soon)"
      aria-label="Toggle AI Chat"
      aria-pressed={$aiChat.isOpen}
      onclick={toggleAiChat}
    >
      <Icon name="sparks" size={13} class="ai-glyph" />
      AI
    </button>
  </div>
</footer>

<style>
  .status-bar {
    height: var(--status-height);
    flex-shrink: 0;
    background-color: var(--background);
    border-top: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 6px 0 12px;
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .status-left,
  .status-right {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .status-item {
    white-space: nowrap;
  }

  /* AI button — clearly interactive (hover/active), but non-functional in W4. */
  .ai-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 18px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.02em;
    cursor: pointer;
    transition: color 0.12s ease, background-color 0.12s ease, border-color 0.12s ease;
  }
  .ai-btn:hover {
    color: var(--foreground);
    background-color: var(--secondary);
  }
  .ai-btn:active {
    background-color: var(--sidebar-accent);
  }
  .ai-btn.active {
    color: var(--primary-foreground);
    background-color: var(--primary);
    border-color: var(--primary);
  }
  .ai-btn :global(.ai-glyph) {
    color: var(--primary);
  }
  .ai-btn.active :global(.ai-glyph) {
    color: var(--primary-foreground);
  }
</style>
