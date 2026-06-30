<script lang="ts">
  import { onMount } from 'svelte'
  import type { LspLanguage, LspStatus } from '../tauri/commands'
  import {
    dismissLspOnboarding,
    loadLspOnboarding,
    lspOnboarding,
  } from '../stores/lsp-onboarding'
  import Icon from './Icon.svelte'

  let {
    path = null,
    status = null,
    onCheckAgain,
  }: {
    path?: string | null
    status?: LspStatus | null
    onCheckAgain?: () => void | Promise<void>
  } = $props()

  let guideOpen = $state(false)
  let selected = $state<LspLanguage>('rust')
  let busy = $state(false)

  const missing = $derived.by(() => (status?.state === 'missing_server' ? status : null))
  const dismissed = $derived(missing ? $lspOnboarding.dismissed[missing.language] : false)
  const visible = $derived(!!path && !!missing && !dismissed)

  const INSTALL_GUIDES: Record<
    LspLanguage,
    { title: string; subtitle: string; commands: { label: string; command: string }[] }
  > = {
    rust: {
      title: 'Rust - rust-analyzer',
      subtitle: 'Installed through rustup.',
      commands: [{ label: 'All OS', command: 'rustup component add rust-analyzer' }],
    },
    typescript: {
      title: 'TypeScript - typescript-language-server',
      subtitle: 'Also powers JavaScript files.',
      commands: [
        {
          label: 'npm',
          command: 'npm install -g typescript-language-server typescript',
        },
      ],
    },
    javascript: {
      title: 'JavaScript - typescript-language-server',
      subtitle: 'JavaScript uses the TypeScript language server bucket.',
      commands: [
        {
          label: 'npm',
          command: 'npm install -g typescript-language-server typescript',
        },
      ],
    },
    python: {
      title: 'Python - Pyright / pylsp',
      subtitle: 'Pyright is the default; pylsp can be configured in Settings.',
      commands: [
        { label: 'Pyright via pip', command: 'pip install pyright' },
        { label: 'Pyright via npm', command: 'npm install -g pyright' },
        { label: 'pylsp fallback', command: 'pip install python-lsp-server' },
      ],
    },
  }

  onMount(() => {
    void loadLspOnboarding().catch(() => {})
  })

  function openGuide() {
    if (missing) selected = missing.language
    guideOpen = true
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node)
    return {
      destroy() {
        if (node.parentNode) node.parentNode.removeChild(node)
      },
    }
  }

  async function dismiss() {
    if (!missing || busy) return
    busy = true
    try {
      await dismissLspOnboarding(missing.language)
    } finally {
      busy = false
    }
  }

  async function checkAgain() {
    if (busy) return
    busy = true
    try {
      await onCheckAgain?.()
      void loadLspOnboarding().catch(() => {})
    } finally {
      busy = false
    }
  }
</script>

{#if visible && missing}
  <div class="lsp-onboarding" role="status">
    <div class="onboarding-copy">
      <strong>{missing.command} not found</strong>
      <span>diagnostics, hover, and completion are disabled for this file.</span>
    </div>
    <div class="onboarding-actions">
      <button type="button" onclick={openGuide}>Show Install Guide</button>
      <button type="button" onclick={dismiss} disabled={busy}>Don't show again</button>
    </div>
  </div>
{/if}

{#if guideOpen}
  <div use:portal class="guide-backdrop" role="presentation" onclick={() => (guideOpen = false)}></div>
  <div use:portal class="guide-modal" role="dialog" aria-modal="true" aria-label="LSP install guide">
    <header class="guide-header">
      <div>
        <h3>LSP Install Guide</h3>
        <p>{INSTALL_GUIDES[selected].subtitle}</p>
      </div>
      <button type="button" class="guide-close" aria-label="Close" onclick={() => (guideOpen = false)}>
        <Icon name="xmark" size={14} />
      </button>
    </header>

    <div class="guide-tabs" role="tablist" aria-label="Languages">
      {#each Object.entries(INSTALL_GUIDES) as [language, guide]}
        <button
          type="button"
          class:selected={selected === language}
          role="tab"
          aria-selected={selected === language}
          onclick={() => (selected = language as LspLanguage)}
        >
          {guide.title.split(' - ')[0]}
        </button>
      {/each}
    </div>

    <section class="guide-body">
      <h4>{INSTALL_GUIDES[selected].title}</h4>
      {#each INSTALL_GUIDES[selected].commands as item}
        <div class="install-command">
          <span>{item.label}</span>
          <code>{item.command}</code>
        </div>
      {/each}
    </section>

    <footer class="guide-footer">
      <button type="button" onclick={checkAgain} disabled={busy}>
        {busy ? 'Checking...' : 'Check Again'}
      </button>
    </footer>
  </div>
{/if}

<style>
  .lsp-onboarding {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 12px 8px 14px;
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--primary) 9%, var(--background));
    color: var(--foreground);
    font-size: 12px;
  }
  .onboarding-copy {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .onboarding-copy strong,
  .onboarding-copy span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .onboarding-copy span {
    color: var(--muted-foreground);
  }
  .onboarding-actions {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: 0 0 auto;
  }
  .onboarding-actions button,
  .guide-footer button,
  .guide-tabs button,
  .guide-close {
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--secondary);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }
  .onboarding-actions button {
    height: 24px;
    padding: 0 9px;
  }
  .onboarding-actions button:hover:not(:disabled),
  .guide-footer button:hover:not(:disabled),
  .guide-tabs button:hover,
  .guide-close:hover {
    border-color: var(--primary);
  }
  button:disabled {
    opacity: 0.65;
    cursor: wait;
  }
  .guide-backdrop {
    position: fixed;
    inset: 0;
    z-index: 96;
    background: rgba(0, 0, 0, 0.42);
  }
  .guide-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 97;
    width: min(560px, calc(100vw - 28px));
    max-height: min(620px, calc(100vh - 32px));
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--popover);
    color: var(--foreground);
    box-shadow: var(--shadow-xl);
  }
  .guide-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 16px 10px;
    border-bottom: 1px solid var(--border);
  }
  .guide-header h3 {
    margin: 0;
    font-size: 14px;
  }
  .guide-header p {
    margin: 5px 0 0;
    color: var(--muted-foreground);
    font-size: 12px;
  }
  .guide-close {
    width: 24px;
    height: 24px;
    padding: 0;
  }
  .guide-tabs {
    display: flex;
    gap: 6px;
    padding: 10px 16px 0;
    overflow-x: auto;
  }
  .guide-tabs button {
    height: 26px;
    padding: 0 10px;
    white-space: nowrap;
  }
  .guide-tabs button.selected {
    border-color: var(--primary);
    background: color-mix(in srgb, var(--primary) 18%, var(--secondary));
  }
  .guide-body {
    padding: 14px 16px;
    overflow-y: auto;
  }
  .guide-body h4 {
    margin: 0 0 10px;
    font-size: 13px;
  }
  .install-command {
    display: grid;
    grid-template-columns: 118px minmax(0, 1fr);
    gap: 10px;
    align-items: start;
    padding: 9px 0;
    border-top: 1px solid var(--border);
  }
  .install-command span {
    color: var(--muted-foreground);
    font-size: 12px;
  }
  .install-command code {
    display: block;
    min-width: 0;
    padding: 7px 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--background);
    color: var(--foreground);
    font-family: var(--font-mono);
    font-size: 12px;
    overflow-x: auto;
    white-space: pre;
  }
  .guide-footer {
    display: flex;
    justify-content: flex-end;
    padding: 10px 16px 14px;
    border-top: 1px solid var(--border);
  }
  .guide-footer button {
    height: 28px;
    padding: 0 12px;
  }
  @media (max-width: 620px) {
    .lsp-onboarding {
      align-items: stretch;
      flex-direction: column;
    }
    .onboarding-copy {
      align-items: flex-start;
      flex-direction: column;
      gap: 3px;
    }
    .onboarding-actions {
      justify-content: flex-start;
      flex-wrap: wrap;
    }
    .install-command {
      grid-template-columns: 1fr;
    }
  }
</style>
