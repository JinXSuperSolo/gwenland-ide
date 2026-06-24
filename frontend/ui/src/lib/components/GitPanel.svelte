<script lang="ts">
  import { git, refreshGit } from '../stores/git'
  import { workspace } from '../stores/workspace'
  import { showSidebarView } from '../stores/sidebar'
  import { openDiff } from '../stores/tabs'
  import {
    gitStage,
    gitUnstage,
    gitDiscard,
    gitCommit,
    gitPush,
    gitPull,
    type GitFileStatus,
  } from '../tauri/commands'
  import Icon from './Icon.svelte'

  // GWEN-328: Source Control panel — staged/unstaged changes, commit, sync.
  const root = $derived($workspace.folderPath)

  let commitMsg = $state('')
  let busy = $state(false)
  let notice = $state<string | null>(null)

  const staged = $derived($git.files.filter((f) => f.staged))
  const unstaged = $derived($git.files.filter((f) => !f.staged))
  const canCommit = $derived(commitMsg.trim().length > 0 && staged.length > 0 && !busy)

  async function withRefresh(fn: () => Promise<unknown>) {
    if (!root) return
    busy = true
    notice = null
    try {
      await fn()
      await refreshGit()
    } catch (e) {
      notice = String(e)
    } finally {
      busy = false
    }
  }

  const stageFile = (f: GitFileStatus) => withRefresh(() => gitStage(root!, f.path))
  const unstageFile = (f: GitFileStatus) => withRefresh(() => gitUnstage(root!, f.path))
  const stageAll = () => withRefresh(() => gitStage(root!, '', true))
  const unstageAll = () => withRefresh(() => gitUnstage(root!, '', true))

  function discardFile(f: GitFileStatus) {
    if (!confirm(`Discard changes to ${f.path}? This cannot be undone.`)) return
    void withRefresh(() => gitDiscard(root!, f.path, f.untracked))
  }

  async function commit() {
    if (!canCommit) return
    await withRefresh(() => gitCommit(root!, commitMsg.trim()))
    if (!notice) commitMsg = ''
  }

  const push = () => withRefresh(() => gitPush(root!))
  const pull = () => withRefresh(() => gitPull(root!))

  function viewDiff(f: GitFileStatus) {
    if (root) openDiff(root, f.path, f.untracked)
  }
</script>

<aside class="git-panel" aria-label="Source Control">
  <header class="panel-header">
    <span class="panel-title">Source Control</span>
    <div class="header-actions">
      <button class="header-btn" title="Refresh" aria-label="Refresh" onclick={() => void refreshGit()}>
        <Icon name="refresh" size={14} />
      </button>
    </div>
  </header>

  <div class="panel-body">
    {#if !$git.isRepo}
      <div class="empty">This folder is not a git repository.</div>
    {:else}
      <!-- COMMIT -->
      <section class="commit">
        <textarea
          class="commit-msg"
          bind:value={commitMsg}
          rows="2"
          placeholder="Message (commit staged changes)"
          aria-label="Commit message"
        ></textarea>
        <button class="commit-btn" disabled={!canCommit} onclick={commit}>
          <Icon name="git-commit" size={14} /> Commit
        </button>
      </section>

      {#if notice}
        <div class="notice">{notice}</div>
      {/if}

      <!-- STAGED -->
      {#if staged.length > 0}
        <section class="group">
          <div class="group-head">
            <span class="group-title">Staged Changes</span>
            <div class="group-actions">
              <span class="count">{staged.length}</span>
              <button class="grp-btn" title="Unstage all" aria-label="Unstage all" onclick={unstageAll}>
                <Icon name="xmark" size={13} />
              </button>
            </div>
          </div>
          {#each staged as f (f.path)}
            <div class="file-row">
              <button class="file-main" onclick={() => viewDiff(f)} title={f.path}>
                <span class="badge s-{f.status}">{f.status}</span>
                <span class="file-name">{f.path}</span>
              </button>
              <div class="file-acts">
                <button class="fa" title="Unstage" aria-label="Unstage" onclick={() => unstageFile(f)}>−</button>
              </div>
            </div>
          {/each}
        </section>
      {/if}

      <!-- CHANGES -->
      {#if unstaged.length > 0}
        <section class="group">
          <div class="group-head">
            <span class="group-title">Changes</span>
            <div class="group-actions">
              <span class="count">{unstaged.length}</span>
              <button class="grp-btn" title="Stage all" aria-label="Stage all" onclick={stageAll}>
                <Icon name="plus" size={13} />
              </button>
            </div>
          </div>
          {#each unstaged as f (f.path)}
            <div class="file-row">
              <button class="file-main" onclick={() => viewDiff(f)} title={f.path}>
                <span class="badge s-{f.status}">{f.status}</span>
                <span class="file-name">{f.path}</span>
              </button>
              <div class="file-acts">
                <button class="fa" title="Discard" aria-label="Discard" onclick={() => discardFile(f)}>
                  <Icon name="xmark" size={12} />
                </button>
                <button class="fa" title="Stage" aria-label="Stage" onclick={() => stageFile(f)}>+</button>
              </div>
            </div>
          {/each}
        </section>
      {/if}

      {#if staged.length === 0 && unstaged.length === 0}
        <div class="empty">No changes.</div>
      {/if}

      <!-- SYNC -->
      <section class="sync">
        <button class="sync-btn" disabled={busy} onclick={pull}>
          <Icon name="arrow-up" size={13} class="rot" /> Pull
        </button>
        <button class="sync-btn" disabled={busy} onclick={push}>
          <Icon name="arrow-up" size={13} /> Push
        </button>
      </section>
    {/if}
  </div>
</aside>

<style>
  .git-panel {
    height: 100%;
    background-color: var(--sidebar);
    border-right: 1px solid var(--sidebar-border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .panel-header {
    height: 38px;
    flex-shrink: 0;
    padding: 0 6px 0 14px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--sidebar-border);
  }
  .panel-title {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: var(--tracking-wider);
    color: var(--muted-foreground);
    font-weight: 700;
  }
  .header-btn {
    background: transparent;
    border: none;
    color: var(--muted-foreground);
    cursor: pointer;
    width: 22px;
    height: 22px;
    border-radius: var(--radius-sm);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .header-btn:hover {
    color: var(--foreground);
    background-color: var(--sidebar-accent);
  }
  .panel-body {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .empty {
    font-size: 12px;
    color: var(--muted-foreground);
    padding: 6px;
  }
  .commit {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .commit-msg {
    width: 100%;
    resize: vertical;
    min-height: 44px;
    box-sizing: border-box;
    padding: 8px;
    font-family: var(--font-sans);
    font-size: 12.5px;
    color: var(--foreground);
    background-color: var(--input);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  .commit-msg:focus {
    outline: none;
    border-color: var(--primary);
  }
  .commit-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 7px 12px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--primary-foreground);
    background-color: var(--primary);
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .commit-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .notice {
    font-size: 11.5px;
    color: var(--destructive);
    padding: 4px 6px;
    background-color: color-mix(in srgb, var(--destructive) 12%, transparent);
    border-radius: var(--radius-sm);
    white-space: pre-wrap;
  }
  .group {
    display: flex;
    flex-direction: column;
  }
  .group-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px 4px 4px;
  }
  .group-title {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: var(--tracking-wider);
    color: var(--muted-foreground);
  }
  .group-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .count {
    font-size: 10px;
    color: var(--muted-foreground);
    background-color: var(--secondary);
    border-radius: 999px;
    padding: 0 6px;
    min-width: 16px;
    text-align: center;
  }
  .grp-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
  }
  .grp-btn:hover {
    color: var(--foreground);
    background-color: var(--sidebar-accent);
  }
  .file-row {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: 5px;
  }
  .file-row:hover {
    background-color: var(--sidebar-accent);
  }
  .file-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 6px;
    background: transparent;
    border: none;
    color: var(--sidebar-foreground);
    font-family: var(--font-sans);
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
  }
  .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl; /* keep the filename visible, truncate the dir prefix */
    text-align: left;
  }
  .badge {
    flex-shrink: 0;
    width: 14px;
    text-align: center;
    font-size: 11px;
    font-weight: 700;
    font-family: var(--font-mono);
  }
  .badge.s-M { color: #e2c08d; }
  .badge.s-U { color: #89d185; }
  .badge.s-A { color: #89d185; }
  .badge.s-D { color: #f14c4c; }
  .badge.s-R { color: #e2c08d; }
  .file-acts {
    display: flex;
    align-items: center;
    gap: 1px;
    padding-right: 4px;
    opacity: 0;
  }
  .file-row:hover .file-acts {
    opacity: 1;
  }
  .fa {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
    font-size: 15px;
    line-height: 1;
  }
  .fa:hover {
    color: var(--foreground);
    background-color: var(--secondary);
  }
  .sync {
    display: flex;
    gap: 8px;
    margin-top: 4px;
    padding-top: 12px;
    border-top: 1px solid var(--sidebar-border);
  }
  .sync-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 7px;
    font-size: 12.5px;
    color: var(--foreground);
    background-color: var(--secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .sync-btn:hover:not(:disabled) {
    border-color: var(--primary);
  }
  .sync-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .sync-btn :global(.rot svg) {
    transform: rotate(180deg);
  }
</style>
