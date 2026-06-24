<script lang="ts">
  import { onMount } from 'svelte'
  import { openFolder, openFolderPath } from '../stores/workspace'
  import { newUntitledFile } from '../stores/tabs'
  import { getRecentProjects, type RecentProject } from '../tauri/commands'
  import Icon from './Icon.svelte'

  // GWEN-321: Full-screen welcome shown when no workspace is open. The sidebar,
  // terminal and AI panel are hidden by App.svelte in this state — this is the
  // entire surface. Opening a folder transitions to the full IDE layout (the
  // workspace store's folderPath flips, App.svelte swaps the layout).
  let recents = $state<RecentProject[]>([])

  onMount(async () => {
    try {
      recents = await getRecentProjects()
    } catch {
      recents = []
    }
  })

  function basename(p: string): string {
    return p.split(/[\\/]/).filter(Boolean).pop() || p
  }

  // The parent directory of a project, for a muted path subtitle.
  function dirname(p: string): string {
    const parts = p.split(/[\\/]/).filter(Boolean)
    parts.pop()
    return parts.join('/') || p
  }
</script>

<div class="welcome gw-anim-fade" aria-label="Welcome">
  <div class="welcome-inner">
    <div class="brand">
      <div class="logo-wrap">
        <img class="logo" src="/logo-dark.png" alt="GwenLand" width="96" height="96" />
        <span class="shimmer" aria-hidden="true"></span>
      </div>
      <h1 class="title">GwenLand IDE</h1>
      <p class="subtitle">A local-first, AI-native code editor.</p>
    </div>

    <div class="actions">
      <button type="button" class="action-btn gw-transition" onclick={() => void openFolder()}>
        <Icon name="folder" class="ab-icon" />Open Folder
      </button>
      <button type="button" class="action-btn gw-transition" onclick={() => newUntitledFile()}>
        <Icon name="page-plus" class="ab-icon" />New File
      </button>
    </div>

    <div class="recent-block">
      <div class="recent-title">Recent Projects</div>
      {#if recents.length === 0}
        <div class="recent-empty">No recent projects yet.</div>
      {:else}
        <div class="recent-list">
          {#each recents.slice(0, 6) as r (r.path)}
            <button
              type="button"
              class="recent-item gw-transition"
              title={r.path}
              onclick={() => void openFolderPath(r.path)}
            >
              <Icon name="clock-rotate-right" size={14} class="ri-icon" />
              <span class="ri-name">{basename(r.path)}</span>
              <span class="ri-path">{dirname(r.path)}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <p class="tip">
      Press <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd> to open the Command Palette
    </p>
  </div>
</div>

<style>
  .welcome {
    height: 100vh;
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: var(--background);
    color: var(--foreground);
    overflow: auto;
    padding: 24px;
  }
  .welcome-inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    max-width: 460px;
    gap: 28px;
  }

  /* --- Brand --- */
  .brand {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
  }
  .logo-wrap {
    position: relative;
    width: 96px;
    height: 96px;
    overflow: hidden;
    border-radius: var(--radius-md);
  }
  .logo {
    display: block;
    width: 96px;
    height: 96px;
    object-fit: contain;
  }
  /* Pure-CSS shimmer: a diagonal highlight band sweeping across the logo. */
  .shimmer {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      105deg,
      transparent 35%,
      rgba(255, 255, 255, 0.28) 50%,
      transparent 65%
    );
    transform: translateX(-120%);
    animation: gw-shimmer 3.2s ease-in-out infinite;
    pointer-events: none;
  }
  @keyframes gw-shimmer {
    0% { transform: translateX(-120%); }
    55%, 100% { transform: translateX(120%); }
  }
  .title {
    margin: 4px 0 0;
    font-size: 24px;
    font-weight: 700;
    letter-spacing: var(--tracking-tight);
    color: var(--foreground);
  }
  .subtitle {
    margin: 0;
    font-size: 13px;
    color: var(--muted-foreground);
  }

  /* --- Action buttons --- */
  .actions {
    display: flex;
    gap: 10px;
    width: 100%;
  }
  .action-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px 14px;
    background-color: var(--secondary);
    color: var(--foreground);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-family: var(--font-sans);
    font-size: 13px;
    cursor: pointer;
  }
  .action-btn:hover {
    background-color: var(--sidebar-accent);
    border-color: var(--primary);
  }
  .action-btn :global(.ab-icon) {
    color: var(--primary);
  }

  /* --- Recents --- */
  .recent-block {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .recent-title {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: var(--tracking-wider);
    color: var(--muted-foreground);
    padding: 0 4px 2px;
  }
  .recent-empty {
    font-size: 12px;
    color: var(--muted-foreground);
    opacity: 0.7;
    padding: 2px 4px;
  }
  .recent-list {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .recent-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 10px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--sidebar-foreground);
    font-family: var(--font-sans);
    font-size: 13px;
    cursor: pointer;
    text-align: left;
  }
  .recent-item :global(.ri-icon) {
    color: var(--muted-foreground);
  }
  .ri-name {
    flex-shrink: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 45%;
  }
  .ri-path {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    color: var(--muted-foreground);
    opacity: 0.7;
    text-align: right;
  }
  .recent-item:hover {
    background-color: var(--sidebar-accent);
    color: var(--primary);
  }
  .recent-item:hover :global(.ri-icon) {
    color: var(--primary);
  }

  /* --- Tip --- */
  .tip {
    margin: 4px 0 0;
    font-size: 12px;
    color: var(--muted-foreground);
  }
  .tip kbd {
    display: inline-block;
    padding: 1px 5px;
    margin: 0 1px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--foreground);
    background-color: var(--secondary);
    border: 1px solid var(--border);
    border-radius: 4px;
  }

  @media (prefers-reduced-motion: reduce) {
    .shimmer {
      animation: none;
      display: none;
    }
  }
</style>
