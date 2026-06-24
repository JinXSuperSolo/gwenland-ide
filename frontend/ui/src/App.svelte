<script lang="ts">
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
  import ContextMenuRoot from './lib/context-menu/ContextMenuRoot.svelte'
  import PromptDialog from './lib/components/PromptDialog.svelte'
  import WelcomeScreen from './lib/components/WelcomeScreen.svelte'
  import { aiChat } from './lib/stores/ai-chat'
  import { workspace } from './lib/stores/workspace'
  import { sidebarView } from './lib/stores/sidebar'
  import { tabs } from './lib/stores/tabs'
  import { dispatchShortcut } from './lib/stores/commands'
  import { handleGlobalContextMenu } from './lib/context-menu/globalContextMenu'

  // GWEN-321: with no folder open AND no tabs open, the app is in the "empty"
  // state — show the full-screen welcome instead of the IDE chrome. Opening a
  // folder (or creating a New File, which opens a tab) transitions to the full
  // IDE layout. The check is reactive, so the swap is automatic.
  const showWelcome = $derived(!$workspace.folderPath && $tabs.tabs.length === 0)

  // Global keyboard shortcuts: let the registry try to handle the combo first.
  function onKeydown(e: KeyboardEvent) {
    dispatchShortcut(e)
  }
</script>

<svelte:window onkeydown={onKeydown} oncontextmenu={handleGlobalContextMenu} />

{#if showWelcome}
  <WelcomeScreen />
  <!-- Overlays still mount over the welcome screen (palette, prompts). -->
  <CommandPalette />
  <SettingsPage />
  <ContextMenuRoot />
  <PromptDialog />
{:else}
<div class="app-shell">
  <MenuBar />
  <div class="shell-main">
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
</style>
