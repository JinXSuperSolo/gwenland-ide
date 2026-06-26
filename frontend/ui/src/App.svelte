<script lang="ts">
  import { onMount } from 'svelte'
  import { panels } from './lib/stores/panels'
  import FileTree from './lib/components/FileTree.svelte'
  import GitPanel from './lib/components/GitPanel.svelte'
  import ActivityBar from './lib/components/ActivityBar.svelte'
  import Workspace from './lib/components/Workspace.svelte'
  import TerminalPanel from './lib/components/TerminalPanel.svelte'
  import StatusBar from './lib/components/StatusBar.svelte'
  import ResizeHandle from './lib/components/ResizeHandle.svelte'
  import RestoreStrip from './lib/components/RestoreStrip.svelte'
  import MenuBar from './lib/components/MenuBar.svelte'
  import CommandPalette from './lib/components/CommandPalette.svelte'
  import SettingsPage from './lib/components/SettingsPage.svelte'
  import AiPanel from './lib/components/AiPanel.svelte'
  import DiffActionBar from './lib/components/DiffActionBar.svelte'
  import LocalHistoryPanel from './lib/components/LocalHistoryPanel.svelte'
  import SimpleDiffPanel from './lib/components/SimpleDiffPanel.svelte'
  import ContextMenuRoot from './lib/context-menu/ContextMenuRoot.svelte'
  import PromptDialog from './lib/components/PromptDialog.svelte'
  import WelcomeScreen from './lib/components/WelcomeScreen.svelte'
  import { aiChat } from './lib/stores/ai-chat'
  import { workspace } from './lib/stores/workspace'
  import { sidebarView } from './lib/stores/sidebar'
  import { activateTab, isEditorTab, isDiffTab, isPreviewTab, openFile, tabs, type Tab } from './lib/stores/tabs'
  import { handleGlobalKeydown } from './lib/commands/keybinding-handler'
  import { initWorkspaceStatePersistence } from './lib/stores/workspace-state'
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
    const onDefinition = (event: Event) => {
      const detail = (event as CustomEvent<{ path: string; line: number }>).detail
      if (!detail?.path) return
      void openFile(detail.path).then(() => {
        window.setTimeout(() => revealLine(detail.line + 1), 0)
      })
    }
    window.addEventListener('gwenland:open-definition', onDefinition)
    return () => window.removeEventListener('gwenland:open-definition', onDefinition)
  })

  function tabTitle(tab: Tab): string {
    return tab.name
  }

  function tabSubtitle(tab: Tab): string {
    if (isEditorTab(tab) || isDiffTab(tab)) return tab.path
    if (isPreviewTab(tab)) return tab.source.kind === 'static-file' ? tab.source.path : tab.source.url
    return ''
  }

  function onKeydown(e: KeyboardEvent): boolean {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Tab') {
      e.preventDefault()
      e.stopPropagation()
      if ($tabs.tabs.length === 0) return true
      if (!quickSwitchOpen) {
        quickSwitchIds = $tabs.mruTabIds.filter((id) => $tabs.tabs.some((tab) => tab.id === id))
        quickSwitchIndex = quickSwitchIds.length > 1 ? 1 : 0
        quickSwitchOpen = true
      } else {
        quickSwitchIndex = (quickSwitchIndex + 1) % Math.max(quickSwitchIds.length, 1)
      }
      const id = quickSwitchIds[quickSwitchIndex]
      if (id) activateTab(id)
      return true
    }
    return handleGlobalKeydown(e)
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
        <div class="panel-slot" style:width={`${$panels.fileTree.size}px`}>
          {#if $sidebarView === 'git'}
            <GitPanel />
          {:else}
            <FileTree />
          {/if}
        </div>
        <ResizeHandle target="fileTree" edge="left" />
      {/if}
    {/if}

    <!-- Right column: Workspace (top, grows) over Terminal (bottom, vertical resize) -->
    <div class="center-column">
      <!-- Floating diff-review action bar over the editor (self-hides when idle). -->
      <DiffActionBar />
      <Workspace />

      <!-- GWEN-325: the terminal requires a workspace (its CWD). With no folder
           open it's hidden entirely — not even the restore strip shows. -->
      {#if $workspace.folderPath}
        {#if $panels.terminal.collapsed}
          <RestoreStrip target="terminal" label="Terminal" orientation="horizontal" />
        {:else}
          <ResizeHandle target="terminal" edge="top" />
          <div class="panel-slot-v" style:height={`${$panels.terminal.size}px`}>
            <TerminalPanel />
          </div>
        {/if}
      {/if}
    </div>

    <!-- Right: AI panel (horizontal resize). Visibility from aiChat.isOpen
         (status-bar toggle); width reuses panels.aiPanel.size. -->
    {#if $aiChat.isOpen}
      <ResizeHandle target="aiPanel" edge="right" />
      <div class="panel-slot" style:width={`${$panels.aiPanel.size}px`}>
        <AiPanel />
      </div>
    {/if}
  </div>

  <StatusBar />

  <!-- Overlays -->
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
