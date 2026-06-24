<script lang="ts">
  import Icon from './Icon.svelte'
  import TerminalInstance from './TerminalInstance.svelte'
  import { collapsePanel } from '../stores/panels'
  import { terminalKill } from '../tauri/commands'
  import {
    terminalSessions,
    createSession,
    activateSession,
    removeSession,
    ensureInitialSession,
  } from '../stores/terminal-sessions'

  // Open with one session the first time the panel mounts. Subsequent mounts
  // (after collapse/restore) keep whatever sessions already exist.
  ensureInitialSession()

  function newSession() {
    createSession()
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
  }
  .tab-strip::-webkit-scrollbar {
    height: 0;
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
