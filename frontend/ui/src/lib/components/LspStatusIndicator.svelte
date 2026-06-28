<script lang="ts">
  import { tabs, isEditorTab } from '../stores/tabs'
  import { lsp, normPath, serverKeyForLanguage, lspOpenPath } from '../stores/lsp'
  import { lspRestart, type LspLanguage, type LspStatus } from '../tauri/commands'
  import { activeDoc } from '../editor/active-editor'

  let open = $state(false)
  let restarting = $state(false)

  const activePath = $derived.by(() => {
    const t = $tabs.tabs.find((x) => x.id === $tabs.activeId)
    return t && isEditorTab(t) ? t.path : null
  })
  const activeKey = $derived(activePath ? normPath(activePath) : null)
  const status = $derived<LspStatus | null>(
    activeKey ? ($lsp.status[activeKey] ?? null) : null
  )
  const diags = $derived(activeKey ? ($lsp.diagnostics[activeKey] ?? []) : [])
  const errorCount = $derived(diags.filter((d) => d.severity === 'error').length)
  const warnCount = $derived(diags.filter((d) => d.severity === 'warning').length)

  const LANG_LABEL: Record<LspLanguage, string> = {
    rust: 'Rust',
    typescript: 'TypeScript',
    javascript: 'JavaScript',
    python: 'Python',
  }

  interface View {
    text: string
    tone: 'ok' | 'warn' | 'error' | 'muted'
    title: string
    detail: string
    action: string | null
  }

  const view = $derived.by<View | null>(() => {
    if (!activePath) return null
    const s = status
    if (!s) {
      return {
        text: 'No LSP',
        tone: 'muted',
        title: 'No language server',
        detail: 'No language server is active for this file.',
        action: null,
      }
    }
    switch (s.state) {
      case 'plain_text':
      case 'unsupported_language':
        return {
          text: 'No LSP',
          tone: 'muted',
          title: 'No language server for this file type',
          detail: 'This file type is not mapped to a language server.',
          action: null,
        }
      case 'disabled':
        return {
          text: `${LANG_LABEL[s.language]} off`,
          tone: 'muted',
          title: 'Language server disabled in Settings',
          detail: `${LANG_LABEL[s.language]} language server is disabled in Settings.`,
          action: null,
        }
      case 'missing_server':
        return {
          text: `${serverName(s.language)} missing`,
          tone: 'warn',
          title: `Language server not found: ${s.command}. Install it or set its path in Settings.`,
          detail: `${s.command} was not found on PATH or at the configured location.`,
          action: 'Check Again',
        }
      case 'starting':
        return {
          text: `${serverName(s.language)} starting`,
          tone: 'muted',
          title: 'Language server starting',
          detail: `${serverName(s.language)} is starting for the active file.`,
          action: 'Restart LSP',
        }
      case 'connected':
        return {
          text: s.server_name || serverName(s.language),
          tone: 'ok',
          title: s.server_name ? `Connected: ${s.server_name}` : 'Language server connected',
          detail: `${s.server_name || serverName(s.language)} is connected.`,
          action: 'Restart LSP',
        }
      case 'crashed':
        return {
          text: `${serverName(s.language)} error`,
          tone: 'error',
          title: s.message || 'Language server crashed',
          detail: s.message || 'The language server exited unexpectedly.',
          action: 'Restart LSP',
        }
    }
  })

  function serverName(language: LspLanguage): string {
    if (language === 'rust') return 'rust-analyzer'
    if (language === 'python') return 'pyright'
    return 'typescript-language-server'
  }

  function onWindowClick() {
    open = false
  }

  function toggleMenu(e: MouseEvent) {
    e.stopPropagation()
    open = !open
  }

  async function onRestart(e?: MouseEvent) {
    e?.stopPropagation()
    const s = status
    if (!s || !('language' in s) || restarting) return
    restarting = true
    try {
      await lspRestart(serverKeyForLanguage(s.language))
      if (activePath) {
        const doc = activeDoc()
        if (doc !== null) await lspOpenPath(activePath, doc)
      }
    } finally {
      restarting = false
      open = false
    }
  }
</script>

<svelte:window onclick={onWindowClick} />

{#if view}
  <span class="lsp-wrap">
    <button
      type="button"
      class="lsp-ind"
      title={view.title}
      aria-haspopup="menu"
      aria-expanded={open}
      onclick={toggleMenu}
    >
      <span class="lsp-dot {view.tone}"></span>
      <span class="lsp-text">{view.text}</span>
      {#if status?.state === 'connected' && (errorCount || warnCount)}
        {#if errorCount}<span class="lsp-count err" title="{errorCount} error(s)">{errorCount}</span>{/if}
        {#if warnCount}<span class="lsp-count warn" title="{warnCount} warning(s)">{warnCount}</span>{/if}
      {/if}
    </button>

    {#if open}
      <div class="lsp-menu" role="menu" aria-label="LSP status">
        <div class="lsp-menu-title">
          <span class="lsp-dot {view.tone}"></span>
          <span>{view.text}</span>
        </div>
        <div class="lsp-detail">{view.detail}</div>
        {#if status && 'language' in status}
          <div class="lsp-meta">{LANG_LABEL[status.language]}</div>
        {/if}
        {#if view.action && status && 'language' in status}
          <button class="lsp-action" type="button" onclick={onRestart} disabled={restarting}>
            {restarting ? 'Working...' : view.action}
          </button>
        {/if}
      </div>
    {/if}
  </span>
{/if}

<style>
  .lsp-wrap {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .lsp-ind {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 18px;
    max-width: 210px;
    padding: 0 6px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 11px;
    line-height: 1;
    white-space: nowrap;
    cursor: pointer;
  }
  .lsp-ind:hover,
  .lsp-ind[aria-expanded='true'] {
    color: var(--foreground);
    background: var(--secondary);
  }
  .lsp-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    background-color: var(--muted-foreground);
  }
  .lsp-dot.ok {
    background-color: var(--chart-2, #3fb950);
  }
  .lsp-dot.warn {
    background-color: var(--chart-4, #d29922);
  }
  .lsp-dot.error {
    background-color: var(--destructive);
  }
  .lsp-dot.muted {
    background-color: var(--muted-foreground);
  }
  .lsp-text {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lsp-count {
    display: inline-flex;
    align-items: center;
    min-width: 14px;
    height: 14px;
    padding: 0 4px;
    border-radius: 7px;
    font-size: 10px;
    font-weight: 700;
    color: var(--background);
  }
  .lsp-count.err {
    background-color: var(--destructive);
  }
  .lsp-count.warn {
    background-color: var(--chart-4, #d29922);
  }
  .lsp-menu {
    position: absolute;
    left: 0;
    bottom: calc(100% + 6px);
    z-index: 65;
    width: min(300px, calc(100vw - 24px));
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--popover);
    color: var(--foreground);
    box-shadow: var(--shadow-lg);
  }
  .lsp-menu-title {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    font-size: 12px;
    font-weight: 700;
  }
  .lsp-menu-title span:last-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lsp-detail {
    margin-top: 8px;
    color: var(--muted-foreground);
    font-size: 12px;
    line-height: 1.45;
    overflow-wrap: anywhere;
  }
  .lsp-meta {
    margin-top: 8px;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 11px;
  }
  .lsp-action {
    margin-top: 10px;
    height: 24px;
    padding: 0 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--secondary);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }
  .lsp-action:hover:not(:disabled) {
    border-color: var(--primary);
  }
  .lsp-action:disabled {
    opacity: 0.65;
    cursor: wait;
  }
</style>
