<script lang="ts">
  import { fade } from 'svelte/transition'
  import { cursor } from '../stores/cursor'
  import { aiChat, toggleAiChat } from '../stores/ai-chat'
  import { cancelStream } from '../ai/ai-chat-setup'
  import { agentic } from '../stores/agentic'
  import { cancelAgentSession } from '../agentic/agentic-setup'
  import { tabs, isEditorTab } from '../stores/tabs'
  import { workspace } from '../stores/workspace'
  import { cancelWorkspaceSearch, workspaceSearch } from '../stores/workspace-search'
  import { git } from '../stores/git'
  import { openPrompt } from '../stores/prompt-dialog'
  import { activeIndentInfo, editorGoToLine } from '../editor/active-editor'
  import { languageIdForFilename } from '../editor/language-detect'
  import { safetyEvaluate, type SafetyDecision } from '../tauri/commands'
  import { editorPreferences } from '../stores/editor-preferences'
  import { openSettings } from '../stores/ui'
  import Icon from './Icon.svelte'
  import LspStatusIndicator from './LspStatusIndicator.svelte'
  import GitStatusBar from './GitStatusBar.svelte'

  let safety = $state<SafetyDecision | null>(null)
  let safetyNonce = 0

  type PerfBadgeTone = 'info' | 'warn' | 'accent'
  type PerfBadge = {
    id: string
    label: string
    title: string
    spinning?: boolean
    tone?: PerfBadgeTone
    onClick?: () => void | Promise<void>
  }

  const activeTab = $derived($tabs.tabs.find((tab) => tab.id === $tabs.activeId) ?? null)
  const activePath = $derived(activeTab && isEditorTab(activeTab) ? activeTab.path : '')
  const largeFile = $derived(!!activeTab && isEditorTab(activeTab) && activeTab.large === true)
  const veryLargeFile = $derived(!!activeTab && isEditorTab(activeTab) && activeTab.veryLarge === true)
  const agentRunning = $derived(
    $aiChat.activeStreamId !== null ||
      $agentic.activeStreamId !== null ||
      $agentic.busy ||
      $agentic.toolActive ||
      $agentic.isRunningCommand
  )
  const language = $derived(activePath ? languageIdForFilename(activePath) ?? 'plaintext' : '')
  const indent = $derived.by(() => {
    void $cursor
    return activeIndentInfo()
  })
  const perfBadges = $derived<PerfBadge[]>(
    [
      {
        id: 'git-scanning',
        label: 'Git',
        title: 'Scanning git status',
        spinning: true,
        tone: 'info' as const,
        active: $git.refreshing,
      },
      {
        id: 'indexing',
        label: 'Indexing',
        title: 'Indexing workspace files',
        spinning: true,
        tone: 'info' as const,
        active: $workspace.loading,
      },
      {
        id: 'large-file',
        label: veryLargeFile ? 'Large File RO' : 'Large File',
        title: veryLargeFile
          ? 'Very large file opened read-only as plain text for performance'
          : 'Large File Mode: expensive editor features are disabled',
        tone: 'warn' as const,
        active: largeFile,
      },
      {
        id: 'low-end',
        label: 'Low-End',
        title: 'Low-End Mode is on. Open Performance settings.',
        tone: 'warn' as const,
        active: $editorPreferences.lowEndMode,
        onClick: openSettings,
      },
      {
        id: 'searching',
        label: 'Searching',
        title: 'Workspace search is running. Click to cancel.',
        spinning: true,
        tone: 'accent' as const,
        active: $workspaceSearch.searching,
        onClick: cancelWorkspaceSearch,
      },
      {
        id: 'agent',
        label: 'AI Running',
        title: 'AI or agent work is running. Click to stop.',
        spinning: true,
        tone: 'accent' as const,
        active: agentRunning,
        onClick: cancelAiWork,
      },
    ]
      .filter((badge) => badge.active)
      .map(({ active: _active, ...badge }) => badge)
  )

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

  async function cancelAiWork() {
    const chatStreamId = $aiChat.activeStreamId
    const agentState = $agentic
    if (chatStreamId) await cancelStream()
    if (
      agentState.session &&
      (agentState.activeStreamId ||
        agentState.busy ||
        agentState.toolActive ||
        agentState.isRunningCommand)
    ) {
      await cancelAgentSession()
    }
  }
</script>

<footer class="status-bar" aria-label="Status Bar">
  <div class="status-left">
    <GitStatusBar />
    <LspStatusIndicator />
  </div>
  <div class="status-right">
    {#if perfBadges.length > 0}
      <div class="perf-badges" aria-label="Performance activity">
        {#each perfBadges as badge (badge.id)}
          {#if badge.onClick}
            <button
              type="button"
              class={`perf-badge tone-${badge.tone ?? 'info'}`}
              title={badge.title}
              aria-label={badge.title}
              onclick={badge.onClick}
              out:fade={{ duration: 300 }}
            >
              {#if badge.spinning}<span class="perf-spinner" aria-hidden="true"></span>{/if}
              <span>{badge.label}</span>
            </button>
          {:else}
            <span
              class={`perf-badge tone-${badge.tone ?? 'info'}`}
              title={badge.title}
              out:fade={{ duration: 300 }}
            >
              {#if badge.spinning}<span class="perf-spinner" aria-hidden="true"></span>{/if}
              <span>{badge.label}</span>
            </span>
          {/if}
        {/each}
      </div>
    {/if}
    {#if safety?.protected_path_matched}
      <span class="status-item protected" title={safety.reason}>Protected</span>
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
    <button
      class="ai-btn"
      class:active={$aiChat.isOpen}
      title="AI Chat"
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
    min-width: 0;
  }
  .status-right {
    justify-content: flex-end;
  }
  .status-item {
    white-space: nowrap;
  }
  .perf-badges {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .perf-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 18px;
    max-width: 126px;
    padding: 0 6px;
    border: 1px solid color-mix(in srgb, currentColor 26%, transparent);
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, currentColor 8%, transparent);
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    line-height: 1;
    white-space: nowrap;
  }
  button.perf-badge {
    cursor: pointer;
  }
  button.perf-badge:hover {
    color: var(--foreground);
    background: color-mix(in srgb, currentColor 13%, transparent);
  }
  .perf-badge span:last-child {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tone-info {
    color: var(--muted-foreground);
  }
  .tone-warn {
    color: #e2c08d;
  }
  .tone-accent {
    color: var(--primary);
  }
  .perf-spinner {
    width: 9px;
    height: 9px;
    flex: 0 0 auto;
    border: 1px solid color-mix(in srgb, currentColor 35%, transparent);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
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
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

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
