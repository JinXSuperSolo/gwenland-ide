<script lang="ts">
  import { cursor } from '../stores/cursor'
  import { aiChat, toggleAiChat } from '../stores/ai-chat'
  import { tabs, isEditorTab } from '../stores/tabs'
  import { workspace } from '../stores/workspace'
  import { openPrompt } from '../stores/prompt-dialog'
  import { activeIndentInfo, editorGoToLine } from '../editor/active-editor'
  import { languageIdForFilename } from '../editor/language-detect'
  import { safetyEvaluate, type SafetyDecision } from '../tauri/commands'
  import Icon from './Icon.svelte'
  import LspStatusIndicator from './LspStatusIndicator.svelte'
  import GitStatusBar from './GitStatusBar.svelte'

  let safety = $state<SafetyDecision | null>(null)
  let safetyNonce = 0

  const activeTab = $derived($tabs.tabs.find((tab) => tab.id === $tabs.activeId) ?? null)
  const activePath = $derived(activeTab && isEditorTab(activeTab) ? activeTab.path : '')
  const language = $derived(activePath ? languageIdForFilename(activePath) ?? 'plaintext' : '')
  const indent = $derived.by(() => {
    void $cursor
    return activeIndentInfo()
  })

  $effect(() => {
    const root = $workspace.folderPath
    const path = activePath
    const nonce = ++safetyNonce
    safety = null
    if (!root || !path) return
    void safetyEvaluate(JSON.stringify({ kind: 'file_read', path }), root, 'user', 'standard')
      .then((decision) => {
        if (nonce === safetyNonce) safety = decision
      })
      .catch(() => {
        if (nonce === safetyNonce) safety = null
      })
  })

  async function openLanguagePicker() {
    if (!activePath) return
    await openPrompt({
      title: 'Language Mode',
      label: 'Language mode',
      initialValue: language,
      confirmLabel: 'OK',
    })
  }
</script>

<footer class="status-bar" aria-label="Status Bar">
  <div class="status-left">
    <GitStatusBar />
    {#if $aiChat.activeStreamId}
      <span class="status-item ai-running"><span class="spinner"></span>AI Running</span>
    {/if}
  </div>
  <div class="status-right">
    {#if safety?.protected_path_matched}
      <span class="status-item protected" title={safety.reason}>🔒 Protected</span>
    {/if}
    {#if safety && safety.risk !== 'safe'}
      <span class={`status-item risk risk-${safety.risk}`}>{safety.risk.toUpperCase()}</span>
    {/if}
    {#if activePath}
      <button type="button" class="status-btn" onclick={openLanguagePicker}>{language}</button>
    {/if}
    {#if $cursor}
      <button type="button" class="status-btn" onclick={editorGoToLine}>
        Ln {$cursor.line}, Col {$cursor.col}
      </button>
      <span class="status-item">UTF-8</span>
      <span class="status-item">{indent?.kind ?? 'Spaces'}: {indent?.size ?? 2}</span>
    {/if}
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
  .status-btn {
    height: 18px;
    padding: 0 6px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 11px;
    cursor: pointer;
  }
  .status-btn:hover {
    color: var(--foreground);
    background: var(--secondary);
  }
  .protected {
    color: var(--foreground);
  }
  .risk-low,
  .risk-medium {
    color: #e2c08d;
  }
  .risk-high,
  .risk-secret,
  .risk-destructive,
  .risk-unknown {
    color: var(--destructive);
  }
  .ai-running {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .spinner {
    width: 9px;
    height: 9px;
    border: 1px solid color-mix(in srgb, var(--primary) 35%, transparent);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
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
