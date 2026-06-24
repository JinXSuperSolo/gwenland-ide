<script lang="ts">
  import { panels } from './lib/stores/panels'
  import FileTree from './lib/components/FileTree.svelte'
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
  import { aiChat } from './lib/stores/ai-chat'
  import { dispatchShortcut } from './lib/stores/commands'
  import { handleGlobalContextMenu } from './lib/context-menu/globalContextMenu'

  // Global keyboard shortcuts: let the registry try to handle the combo first.
  function onKeydown(e: KeyboardEvent) {
    dispatchShortcut(e)
  }
</script>

<svelte:window onkeydown={onKeydown} oncontextmenu={handleGlobalContextMenu} />

<div class="app-shell">
  <MenuBar />
  <div class="shell-main">
    <!-- Left: File Tree (horizontal resize) -->
    {#if $panels.fileTree.collapsed}
      <RestoreStrip target="fileTree" label="File Tree" orientation="vertical" />
    {:else}
      <div class="panel-slot" style:width={`${$panels.fileTree.size}px`}>
        <FileTree />
      </div>
      <ResizeHandle target="fileTree" edge="left" />
    {/if}

    <!-- Right column: Workspace (top, grows) over Terminal (bottom, vertical resize) -->
    <div class="center-column">
      <!-- Floating diff-review action bar over the editor (self-hides when idle). -->
      <DiffActionBar />
      <Workspace />

      {#if $panels.terminal.collapsed}
        <RestoreStrip target="terminal" label="Terminal" orientation="horizontal" />
      {:else}
        <ResizeHandle target="terminal" edge="top" />
        <div class="panel-slot-v" style:height={`${$panels.terminal.size}px`}>
          <TerminalPanel />
        </div>
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
