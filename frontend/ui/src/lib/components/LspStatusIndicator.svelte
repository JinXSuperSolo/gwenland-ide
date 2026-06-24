<script lang="ts">
  // Compact, non-modal LSP status for the active editor file (Requirement 12.x).
  // Shows the language + connection state, diagnostic counts when connected, and
  // a Restart action when the server has crashed.
  import { tabs, isEditorTab } from '../stores/tabs'
  import { lsp, serverKeyForLanguage, lspOpenPath } from '../stores/lsp'
  import { lspRestart, type LspLanguage, type LspStatus } from '../tauri/commands'
  import { activeDoc } from '../editor/active-editor'

  const activePath = $derived.by(() => {
    const t = $tabs.tabs.find((x) => x.id === $tabs.activeId)
    return t && isEditorTab(t) ? t.path : null
  })
  const status = $derived<LspStatus | null>(
    activePath ? ($lsp.status[activePath] ?? null) : null
  )
  const diags = $derived(activePath ? ($lsp.diagnostics[activePath] ?? []) : [])
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
    restart: boolean
  }

  const view = $derived.by<View | null>(() => {
    if (!activePath) return null
    const s = status
    if (!s) return { text: 'Plain Text', tone: 'muted', title: 'No language server', restart: false }
    switch (s.state) {
      case 'plain_text':
      case 'unsupported_language':
        return {
          text: 'Plain Text',
          tone: 'muted',
          title: 'Plain text — no language server for this file type',
          restart: false,
        }
      case 'disabled':
        return {
          text: `${LANG_LABEL[s.language]} · off`,
          tone: 'muted',
          title: 'Language server disabled in Settings',
          restart: false,
        }
      case 'missing_server':
        return {
          text: `${LANG_LABEL[s.language]} · no server`,
          tone: 'warn',
          title: `Language server not found: ${s.command}. Install it or set its path in Settings.`,
          restart: false,
        }
      case 'starting':
        return {
          text: `${LANG_LABEL[s.language]} · starting…`,
          tone: 'muted',
          title: 'Language server starting',
          restart: false,
        }
      case 'connected':
        return {
          text: LANG_LABEL[s.language],
          tone: 'ok',
          title: s.server_name ? `Connected · ${s.server_name}` : 'Language server connected',
          restart: false,
        }
      case 'crashed':
        return {
          text: `${LANG_LABEL[s.language]} · crashed`,
          tone: 'error',
          title: s.message || 'Language server crashed',
          restart: true,
        }
    }
  })

  async function onRestart() {
    const s = status
    if (!s || !('language' in s)) return
    await lspRestart(serverKeyForLanguage(s.language))
    // Re-open the active document so the fresh server gets its state back.
    if (activePath) {
      const doc = activeDoc()
      if (doc !== null) await lspOpenPath(activePath, doc)
    }
  }
</script>

{#if view}
  <span class="lsp-ind" title={view.title}>
    <span class="lsp-dot {view.tone}"></span>
    <span class="lsp-text">{view.text}</span>
    {#if status?.state === 'connected' && (errorCount || warnCount)}
      {#if errorCount}<span class="lsp-count err" title="{errorCount} error(s)">{errorCount}</span>{/if}
      {#if warnCount}<span class="lsp-count warn" title="{warnCount} warning(s)">{warnCount}</span>{/if}
    {/if}
    {#if view.restart}
      <button class="lsp-restart" onclick={onRestart} title="Restart language server">Restart</button>
    {/if}
  </span>
{/if}

<style>
  .lsp-ind {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
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
    color: var(--muted-foreground);
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
  .lsp-restart {
    height: 16px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
  }
  .lsp-restart:hover {
    background-color: var(--secondary);
  }
</style>
