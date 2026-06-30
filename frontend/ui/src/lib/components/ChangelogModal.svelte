<script lang="ts">
  import { changelogOpen, closeChangelog } from '../stores/ui'
  import Icon from './Icon.svelte'

  // Escape is owned by the centralized overlay stack (App.svelte → closeTopmost),
  // so this modal no longer listens for it directly (that caused stacked overlays
  // to all close on a single press).

  const updates = [
    {
      version: '0.1.0',
      date: 'June 30, 2026',
      changes: [
        'Added intelligent breadcrumb truncation for deep workspaces',
        'Introduced global hover system for unified UI feedback',
        'Replaced standard OS dialogs with custom themed modals',
        'Resolved layout shifts in the AI Chat history pane',
        'Implemented horizontal scroll for the editor tab bar'
      ]
    },
    {
      version: '0.0.9',
      date: 'June 15, 2026',
      changes: [
        'Initial beta release of GwenLand IDE',
        'Integrated Tauri + Rust backend for native performance',
        'CodeMirror 6 integration with custom dark theme'
      ]
    }
  ]
</script>

{#if $changelogOpen}
  <div
    class="overlay gw-anim-overlay"
    role="presentation"
    onclick={(e) => { if (e.target === e.currentTarget) closeChangelog() }}
  >
    <div class="dialog gw-anim-pop" role="dialog" aria-modal="true" aria-label="Changelog">
      <div class="dialog-header">
        <div class="dialog-title">What's New</div>
        <button type="button" class="close-btn" aria-label="Close" onclick={closeChangelog}>
          <Icon name="xmark" size={16} />
        </button>
      </div>
      <div class="dialog-body">
        {#each updates as update}
          <div class="update-block">
            <div class="update-meta">
              <span class="update-version">v{update.version}</span>
              <span class="update-date">{update.date}</span>
            </div>
            <ul class="update-list">
              {#each update.changes as change}
                <li>{change}</li>
              {/each}
            </ul>
          </div>
        {/each}
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
    width: 480px;
    max-width: 90vw;
    max-height: 80vh;
    background-color: var(--popover);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-xl);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
    background-color: var(--card);
  }
  .dialog-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--foreground);
  }
  .close-btn {
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    border-radius: var(--radius-sm);
    transition: background-color 0.12s ease;
  }
  .close-btn:hover {
    background-color: var(--hover-bg);
    color: var(--foreground);
  }
  .dialog-body {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .update-block {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .update-meta {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .update-version {
    font-size: 13px;
    font-weight: 700;
    color: var(--primary);
  }
  .update-date {
    font-size: 12px;
    color: var(--muted-foreground);
  }
  .update-list {
    margin: 0;
    padding-left: 20px;
    color: var(--foreground);
    font-size: 13px;
    line-height: 1.6;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .update-list li {
    color: var(--foreground);
  }
  .update-list li::marker {
    color: var(--muted-foreground);
  }
</style>
