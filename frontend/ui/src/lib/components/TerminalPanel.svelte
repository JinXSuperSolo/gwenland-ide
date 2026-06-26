<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './Icon.svelte'
  import TerminalInstance from './TerminalInstance.svelte'
  import CustomDropdown, { type CustomDropdownItem } from './CustomDropdown.svelte'
  import { collapsePanel } from '../stores/panels'
  import {
    terminalDetectShells,
    terminalKill,
    workspaceLoadSettings,
    workspaceSaveSettings,
    type TerminalShellInfo,
  } from '../tauri/commands'
  import { workspace } from '../stores/workspace'
  import {
    terminalSessions,
    createSession,
    activateSession,
    removeSession,
    ensureInitialSession,
  } from '../stores/terminal-sessions'

  // Inline SVG icons for common shells (16×16 viewBox, currentColor strokes).
  const SHELL_ICONS: Record<string, string> = {
    powershell: `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect x="1" y="2" width="12" height="10" rx="1.5" stroke="currentColor" stroke-width="1.2"/>
      <path d="M4 5L7 7L4 9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M7.5 9H10.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
    </svg>`,
    cmd: `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect x="1" y="2" width="12" height="10" rx="1.5" stroke="currentColor" stroke-width="1.2"/>
      <path d="M3.5 5L5.5 7L3.5 9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M6.5 9H10" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
    </svg>`,
    wsl: `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
      <circle cx="7" cy="7" r="5.5" stroke="currentColor" stroke-width="1.2"/>
      <path d="M4.5 9L6 5L7.5 8.5L9 6.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>`,
    bash: `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect x="1" y="2" width="12" height="10" rx="1.5" stroke="currentColor" stroke-width="1.2"/>
      <path d="M3.5 5L6 7L3.5 9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M7 9H10.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
    </svg>`,
    zsh: `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect x="1" y="2" width="12" height="10" rx="1.5" stroke="currentColor" stroke-width="1.2"/>
      <path d="M4 5H9L4 9H9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>`,
    node: `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d="M7 1.5L12 4.5V9.5L7 12.5L2 9.5V4.5L7 1.5Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>
    </svg>`,
    python: `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d="M5 1.5C3.5 1.5 2.5 2.5 2.5 4V6H7V7H1.5C1 7 0.5 7.5 0.5 8V10C0.5 11.5 1.5 12.5 3 12.5H5" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>
      <path d="M9 12.5C10.5 12.5 11.5 11.5 11.5 10V8H7V7H12.5C13 7 13.5 6.5 13.5 6V4C13.5 2.5 12.5 1.5 11 1.5H9" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>
    </svg>`,
  }

  function shellIcon(shell: TerminalShellInfo): string {
    const key = shell.label.toLowerCase()
    for (const [name, svg] of Object.entries(SHELL_ICONS)) {
      if (key.includes(name)) return svg
    }
    // Generic terminal icon fallback
    return `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect x="1" y="2" width="12" height="10" rx="1.5" stroke="currentColor" stroke-width="1.2"/>
      <path d="M3.5 5L6 7L3.5 9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>`
  }

  // Build CustomDropdown items from detected shells.
  const shellItems = $derived<CustomDropdownItem[]>(
    shells.map((s) => ({ value: s.command, label: s.label, svgIcon: shellIcon(s) }))
  )

  // Open with one session the first time the panel mounts. Subsequent mounts
  // (after collapse/restore) keep whatever sessions already exist.
  ensureInitialSession()
  let shells = $state<TerminalShellInfo[]>([])
  let selectedShellCommand = $state<string>('')
  const selectedShell = $derived(
    shells.find((shell) => shell.command === selectedShellCommand) ?? shells[0] ?? null,
  )

  onMount(async () => {
    try {
      shells = await terminalDetectShells()
      const root = $workspace.folderPath
      const saved = root ? await workspaceLoadSettings(root).catch(() => null) : null
      selectedShellCommand =
        saved?.last_terminal_shell && shells.some((shell) => shell.command === saved.last_terminal_shell)
          ? saved.last_terminal_shell
          : shells[0]?.command ?? ''
    } catch {
      shells = []
      selectedShellCommand = ''
    }
  })

  function newSession() {
    createSession(null, selectedShell)
  }

  async function selectShell(command: string) {
    selectedShellCommand = command
    const root = $workspace.folderPath
    if (!root) return
    const current = await workspaceLoadSettings(root).catch(() => ({}))
    await workspaceSaveSettings(root, { ...current, last_terminal_shell: command }).catch(() => {})
  }

  // Close a tab: drop it from the store, then kill its PTY (the store hands back
  // the ptyId so it stays Tauri-free). If that emptied the panel, collapse it.
  function closeSession(key: string) {
    const ptyId = removeSession(key)
    if (ptyId) void terminalKill(ptyId).catch(() => {})
    if ($terminalSessions.sessions.length === 0) collapsePanel('terminal')
  }
</script>

<aside class="terminal" aria-label="Terminal">
  <header class="panel-header">
    <div class="tab-strip" role="tablist">
      {#each $terminalSessions.sessions as session (session.key)}
        <div
          class="term-tab"
          class:active={session.key === $terminalSessions.activeKey}
          role="tab"
          aria-selected={session.key === $terminalSessions.activeKey}
          tabindex="0"
          title={session.title}
          onmousedown={() => activateSession(session.key)}
          onkeydown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault()
              activateSession(session.key)
            }
          }}
        >
          <Icon name="terminal" size={13} />
          <span class="term-tab-title">{session.title}</span>
          <button
            type="button"
            class="term-tab-close"
            title="Kill terminal"
            aria-label={`Kill ${session.title}`}
            onmousedown={(e) => e.stopPropagation()}
            onclick={(e) => {
              e.stopPropagation()
              closeSession(session.key)
            }}
          ><Icon name="xmark" size={12} /></button>
        </div>
      {/each}
    </div>

    <div class="header-actions">
      {#if shells.length}
        <div class="shell-dropdown-wrap">
          <CustomDropdown
            items={shellItems}
            value={selectedShellCommand}
            onSelect={(v) => void selectShell(v)}
            label="Terminal shell"
            compact
          />
        </div>
      {/if}
      <button
        class="header-btn"
        title="New Terminal"
        aria-label="New Terminal"
        onclick={newSession}
      ><Icon name="plus" size={14} /></button>
      <button
        class="header-btn"
        title="Collapse Terminal"
        aria-label="Collapse Terminal"
        onclick={() => collapsePanel('terminal')}
      >×</button>
    </div>
  </header>

  <div class="panel-body">
    <!-- Every session stays mounted; only the active one is visible, so
         scrollback and running processes are preserved across switches. -->
    {#each $terminalSessions.sessions as session (session.key)}
      <TerminalInstance
        key={session.key}
        visible={session.key === $terminalSessions.activeKey}
      />
    {/each}
  </div>
</aside>

<style>
  .terminal {
    height: 100%;
    background-color: var(--card);
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .panel-header {
    height: 38px;
    flex-shrink: 0;
    padding: 0 8px 0 6px;
    display: flex;
    align-items: stretch;
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
    gap: 8px;
  }
  .tab-strip {
    display: flex;
    align-items: stretch;
    overflow-x: auto;
    overflow-y: hidden;
    min-width: 0;
    scrollbar-width: thin;
    scrollbar-color: var(--border) transparent;
  }
  .tab-strip::-webkit-scrollbar {
    height: 4px;
  }
  .tab-strip::-webkit-scrollbar-thumb {
    background: var(--border);
    border-radius: 2px;
  }
  .term-tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px 0 10px;
    border: none;
    background: transparent;
    border-bottom: 2px solid transparent;
    color: var(--muted-foreground);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
    max-width: 160px;
    transition: color 0.15s ease, background-color 0.15s ease;
  }
  .term-tab:hover {
    color: var(--foreground);
    background-color: var(--secondary);
  }
  .term-tab.active {
    color: var(--foreground);
    background-color: var(--background);
    border-bottom: 2px solid var(--primary);
    font-weight: 500;
  }
  .term-tab-title {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .term-tab-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    padding: 0;
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
    border-radius: var(--radius-sm);
    opacity: 0;
    transition: opacity 0.15s ease, background-color 0.15s ease;
  }
  .term-tab:hover .term-tab-close,
  .term-tab.active .term-tab-close {
    opacity: 1;
  }
  .term-tab-close:hover {
    background-color: var(--border);
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }
  .shell-dropdown-wrap {
    max-width: 140px;
    min-width: 80px;
  }
  .header-btn {
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    font-size: 16px;
    line-height: 1;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
  }
  .header-btn:hover {
    color: var(--foreground);
    background-color: var(--sidebar-accent);
  }
  .panel-body {
    flex: 1;
    min-height: 0;
    position: relative;
    overflow: hidden;
    background-color: #1c1c1c;
    padding: 6px 8px;
  }
  /* Instances stack in the same area; hidden ones are display:none. The active
     one fills the body. */
  .panel-body :global(.term-instance) {
    position: absolute;
    inset: 6px 8px;
  }
</style>
