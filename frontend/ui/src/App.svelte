<script lang="ts">
  import { onMount } from 'svelte'
  import { panels } from './lib/stores/panels'
  import GitPanel from './lib/components/GitPanel.svelte'
  import SearchPanel from './lib/components/SearchPanel.svelte'
  import SidebarTabs from './lib/components/SidebarTabs.svelte'
  import ActivityBar from './lib/components/ActivityBar.svelte'
  import Workspace from './lib/components/Workspace.svelte'
  import TerminalPanel from './lib/components/TerminalPanel.svelte'
  import StatusBar from './lib/components/StatusBar.svelte'
  import ResizeHandle from './lib/components/ResizeHandle.svelte'
  import RestoreStrip from './lib/components/RestoreStrip.svelte'
  import MenuBar from './lib/components/MenuBar.svelte'
  import CommandPalette from './lib/components/CommandPalette.svelte'

  import SettingsPage from './lib/components/SettingsPage.svelte'
  import DiffActionBar from './lib/components/DiffActionBar.svelte'
  import LocalHistoryPanel from './lib/components/LocalHistoryPanel.svelte'
  import SimpleDiffPanel from './lib/components/SimpleDiffPanel.svelte'
  import GitGraphWindow from './lib/components/git/GitGraphWindow.svelte'
  import ContextMenuRoot from './lib/context-menu/ContextMenuRoot.svelte'
  import PromptDialog from './lib/components/PromptDialog.svelte'
  import WelcomeScreen from './lib/components/WelcomeScreen.svelte'
  import AboutDialog from './lib/components/AboutDialog.svelte'
  import ChangelogModal from './lib/components/ChangelogModal.svelte'
  import { workspace } from './lib/stores/workspace'
  import { initSidebarTabPersistence, sidebarView } from './lib/stores/sidebar'
  import { activateTab, isCommitDiffTab, isEditorTab, isDiffTab, isGitGraphTab, isPreviewTab, openFile, tabs, type Tab } from './lib/stores/tabs'
  import { handleGlobalKeydown } from './lib/commands/keybinding-handler'
  import { initWorkspaceStatePersistence } from './lib/stores/workspace-state'
  import { initOverlayStack } from './lib/ui/overlay-setup'
  import { closeTopmost } from './lib/stores/overlay-stack'
  import {
    focusedPane,
    nextPane,
    shouldCyclePanes,
    setFocusedPane,
    type Pane,
  } from './lib/stores/pane-focus'
  import { getTerminalHandle } from './lib/terminal/terminal-registry'
  import { terminalSessions } from './lib/stores/terminal-sessions'
  import { canSwitchTabs, initialSwitchIndex, stepSwitchIndex } from './lib/stores/tab-cycle'
  import { handleGlobalContextMenu } from './lib/context-menu/globalContextMenu'
  import { revealLine } from './lib/editor/active-editor'
  import FileIcon from './lib/components/FileIcon.svelte'

  // GWEN-321: with no folder open AND no tabs open, the app is in the "empty"
  // state — show the full-screen welcome instead of the IDE chrome. Opening a
  // folder (or creating a New File, which opens a tab) transitions to the full
  // IDE layout. The check is reactive, so the swap is automatic.
  const showWelcome = $derived(!$workspace.folderPath && $tabs.tabs.length === 0)
  let quickSwitchOpen = $state(false)
  let quickSwitchIds = $state<string[]>([])
  let quickSwitchIndex = $state(0)

  const quickSwitchTabs = $derived.by(() =>
    quickSwitchIds
      .map((id) => $tabs.tabs.find((tab) => tab.id === id))
      .filter((tab): tab is Tab => !!tab),
  )

  onMount(() => {
    initWorkspaceStatePersistence()
    initSidebarTabPersistence()
    const disposeOverlays = initOverlayStack()
    const onDefinition = (event: Event) => {
      const detail = (event as CustomEvent<{ path: string; line: number }>).detail
      if (!detail?.path) return
      void openFile(detail.path).then(() => {
        window.setTimeout(() => revealLine(detail.line + 1), 0)
      })
    }
    window.addEventListener('gwenland:open-definition', onDefinition)
    // Keep the pane-focus indicator honest when focus moves by click/programmatic
    // means (not just Tab): map the focused element to its enclosing pane.
    const onFocusIn = (event: FocusEvent) => {
      const target = event.target as Element | null
      const paneEl = target?.closest?.('[data-pane]') as HTMLElement | null
      setFocusedPane((paneEl?.dataset.pane as Pane | undefined) ?? null)
    }
    window.addEventListener('focusin', onFocusIn)
    return () => {
      window.removeEventListener('gwenland:open-definition', onDefinition)
      window.removeEventListener('focusin', onFocusIn)
      disposeOverlays()
    }
  })

  function tabTitle(tab: Tab): string {
    return tab.name
  }

  function tabSubtitle(tab: Tab): string {
    if (isEditorTab(tab) || isDiffTab(tab)) return tab.path
    if (isGitGraphTab(tab)) return tab.workspacePath
    if (isCommitDiffTab(tab)) return `${tab.shortHash} - ${tab.workspacePath}`
    if (isPreviewTab(tab)) return tab.source.kind === 'static-file' ? tab.source.path : tab.source.url
    return ''
  }

  function onKeydown(e: KeyboardEvent): boolean {
    // Escape closes exactly the topmost overlay (one layer per press). If nothing
    // is open it's a no-op and we let the event fall through (e.g. CodeMirror's
    // own Escape handling stays intact). Plain Escape only — modified Escape is
    // left for editor/search bindings.
    if (e.key === 'Escape' && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
      if (closeTopmost()) {
        e.preventDefault()
        e.stopPropagation()
        return true
      }
      return false
    }

    // Ctrl+Tab / Ctrl+Shift+Tab: cycle open editor tabs via the MRU quick switcher,
    // regardless of which pane has focus. No-op with 0 or 1 open tab.
    if ((e.ctrlKey || e.metaKey) && e.key === 'Tab') {
      e.preventDefault()
      e.stopPropagation()
      const liveIds = $tabs.mruTabIds.filter((id) => $tabs.tabs.some((tab) => tab.id === id))
      if (!canSwitchTabs(liveIds.length)) return true
      if (!quickSwitchOpen) {
        quickSwitchIds = liveIds
        quickSwitchIndex = initialSwitchIndex(quickSwitchIds.length, e.shiftKey)
        quickSwitchOpen = true
      } else {
        quickSwitchIndex = stepSwitchIndex(quickSwitchIndex, quickSwitchIds.length, e.shiftKey)
      }
      const id = quickSwitchIds[quickSwitchIndex]
      if (id) activateTab(id)
      return true
    }

    // Plain Tab / Shift+Tab: cycle focus across major panes (Sidebar → Editor →
    // Terminal). Suppressed in the welcome screen (no IDE chrome), and while focus
    // is inside the editor or a text input so in-editor Tab (indentation) and form
    // Tab still work.
    if (e.key === 'Tab' && !e.ctrlKey && !e.metaKey && !e.altKey && !showWelcome) {
      if (!shouldCyclePanes(document.activeElement)) return false
      const target = nextPane($focusedPane, e.shiftKey ? -1 : 1, paneAvailability())
      if (!target) return false
      e.preventDefault()
      e.stopPropagation()
      focusPane(target)
      return true
    }

    return handleGlobalKeydown(e)
  }

  /** Snapshot which panes are currently focusable for the Tab cycle. */
  function paneAvailability() {
    const hasWorkspace = !!$workspace.folderPath
    return {
      sidebar: hasWorkspace,
      editor: true,
      terminal: hasWorkspace && !$panels.terminal.collapsed,
    }
  }

  /** Move keyboard focus to a pane container (or a focusable child) + mark it. */
  function focusPane(pane: Pane): void {
    setFocusedPane(pane)
    if (pane === 'terminal') {
      getTerminalHandle($terminalSessions.activeKey ?? undefined)?.focus()
      return
    }
    if (pane === 'editor') {
      const cm = document.querySelector<HTMLElement>('.editor-host .cm-content')
      if (cm) {
        cm.focus({ preventScroll: true })
        return
      }
    }
    if (pane === 'sidebar') {
      // Prefer the file-tree viewport so arrow navigation works immediately.
      const tree = document.querySelector<HTMLElement>('.tree-viewport')
      if (tree) {
        tree.focus({ preventScroll: true })
        return
      }
    }
    const el = document.querySelector<HTMLElement>(`[data-pane="${pane}"]`)
    el?.focus({ preventScroll: true })
  }

  function onKeyup(e: KeyboardEvent): void {
    if (quickSwitchOpen && (e.key === 'Control' || e.key === 'Meta')) {
      quickSwitchOpen = false
      quickSwitchIds = []
      quickSwitchIndex = 0
    }
  }
</script>

<svelte:window
  onkeydown={onKeydown}
  onkeyup={onKeyup}
  onblur={() => {
    quickSwitchOpen = false
  }}
  oncontextmenu={handleGlobalContextMenu}
/>

{#if showWelcome}
  <WelcomeScreen />
  <!-- Overlays still mount over the welcome screen (palette, prompts). -->
  <CommandPalette />
  <SettingsPage />
  <ContextMenuRoot />
  <PromptDialog />
  <LocalHistoryPanel />
  <SimpleDiffPanel />
  {#if quickSwitchOpen && quickSwitchTabs.length}
    <div class="quick-switcher" role="listbox" aria-label="Open Editors">
      {#each quickSwitchTabs as tab, index (tab.id)}
        <div
          class="quick-switch-row"
          class:active={index === quickSwitchIndex}
          role="option"
          aria-selected={index === quickSwitchIndex}
        >
          <FileIcon name={tabTitle(tab)} size={17} />
          <span class="qs-main">{tabTitle(tab)}</span>
          <span class="qs-sub">{tabSubtitle(tab)}</span>
        </div>
      {/each}
    </div>
  {/if}
{:else}
<div class="app-shell">
  <MenuBar />
  <div class="shell-main">
    <!-- The whole left sidebar (activity rail + Explorer/Source Control) only
         exists once a folder is explicitly opened. A New File with no workspace
         keeps it hidden — the editor takes the full width. -->
    {#if $workspace.folderPath}
      <!-- Activity rail: switch the left sidebar between Explorer / Source Control. -->
      <ActivityBar />

      <!-- Left sidebar (horizontal resize): Explorer or Source Control. -->
      {#if $panels.fileTree.collapsed}
        <RestoreStrip target="fileTree" label="Sidebar" orientation="vertical" />
      {:else}
        <div
          class="panel-slot"
          class:pane-focused={$focusedPane === 'sidebar'}
          style:width={`${$panels.fileTree.size}px`}
          data-pane="sidebar"
          tabindex="-1"
        >
          {#if $sidebarView === 'git'}
            <GitPanel />
          {:else if $sidebarView === 'search'}
            <SearchPanel />
          {:else}
            <SidebarTabs />
          {/if}
        </div>
        <ResizeHandle target="fileTree" edge="left" />
      {/if}
    {/if}

    <!-- Right column: Workspace (top, grows) over Terminal (bottom, vertical resize) -->
    <div class="center-column">
      <!-- Floating diff-review action bar over the editor (self-hides when idle). -->
      <DiffActionBar />
      <div
        class="editor-pane-wrap"
        class:pane-focused={$focusedPane === 'editor'}
        data-pane="editor"
        tabindex="-1"
      >
        <Workspace />
      </div>

      <!-- GWEN-325: the terminal requires a workspace (its CWD). With no folder
           open it's hidden entirely — not even the restore strip shows. -->
      {#if $workspace.folderPath}
        {#if $panels.terminal.collapsed}
          <RestoreStrip target="terminal" label="Terminal" orientation="horizontal" />
        {:else}
          <ResizeHandle target="terminal" edge="top" />
          <div
            class="panel-slot-v"
            class:pane-focused={$focusedPane === 'terminal'}
            style:height={`${$panels.terminal.size}px`}
            data-pane="terminal"
            tabindex="-1"
          >
            <TerminalPanel />
          </div>
        {/if}
      {/if}
    </div>
  </div>

  <StatusBar />

  <!-- Overlays -->
  <CommandPalette />
  <SettingsPage />
  <ContextMenuRoot />
  <PromptDialog />
  <LocalHistoryPanel />
  <SimpleDiffPanel />
  <GitGraphWindow />
  <AboutDialog />
  <ChangelogModal />
  {#if quickSwitchOpen && quickSwitchTabs.length}
    <div class="quick-switcher" role="listbox" aria-label="Open Editors">
      {#each quickSwitchTabs as tab, index (tab.id)}
        <div
          class="quick-switch-row"
          class:active={index === quickSwitchIndex}
          role="option"
          aria-selected={index === quickSwitchIndex}
        >
          <FileIcon name={tabTitle(tab)} size={17} />
          <span class="qs-main">{tabTitle(tab)}</span>
          <span class="qs-sub">{tabSubtitle(tab)}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
{/if}

<style>
  .app-shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background-color: var(--background);
    color: var(--foreground);
  }
  .shell-main {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }
  /* File Tree slot (horizontal). */
  .panel-slot {
    flex-shrink: 0;
    height: 100%;
    overflow: hidden;
  }
  /* The editor wrapper carries the pane-focus ring; it must not disturb the
     Workspace's flex sizing, so it grows like the Workspace it replaces. */
  .editor-pane-wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    outline: none;
  }
  /* Tab pane-cycling focus indicator (M-keynav §3). A subtle inset ring in the
     GwenLand orange accent (the existing --primary / --ring token) — no new
     color introduced. */
  .pane-focused {
    outline: 2px solid color-mix(in srgb, var(--primary) 70%, transparent);
    outline-offset: -2px;
  }
  /* The center column stacks Workspace + Terminal vertically and takes the
     remaining horizontal space after File Tree. */
  .center-column {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    /* Positioning context for the floating diff-review action bar. */
    position: relative;
  }
  /* Terminal slot (vertical). */
  .panel-slot-v {
    flex-shrink: 0;
    width: 100%;
    overflow: hidden;
  }
  .quick-switcher {
    position: fixed;
    left: 50%;
    top: 92px;
    transform: translateX(-50%);
    z-index: 120;
    width: min(520px, calc(100vw - 32px));
    max-height: min(420px, calc(100vh - 140px));
    overflow: auto;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--popover);
    box-shadow: var(--shadow-lg);
  }
  .quick-switch-row {
    display: grid;
    grid-template-columns: 20px minmax(80px, 0.8fr) minmax(120px, 1.2fr);
    align-items: center;
    gap: 8px;
    min-height: 32px;
    padding: 5px 8px;
    border-radius: var(--radius-sm);
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 12px;
  }
  .quick-switch-row.active {
    color: var(--foreground);
    background: color-mix(in srgb, var(--primary) 16%, var(--secondary));
  }
  .qs-main,
  .qs-sub {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .qs-main {
    font-weight: 600;
  }
  .qs-sub {
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 11px;
  }
</style>
