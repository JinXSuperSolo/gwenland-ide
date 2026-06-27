<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { workspace } from '../stores/workspace'
  import {
    cancelWorkspaceSearch,
    clearWorkspaceSearchResults,
    runWorkspaceSearch,
    setWorkspaceSearchQuery,
    workspaceSearch,
    workspaceSearchGroups,
  } from '../stores/workspace-search'
  import type { WorkspaceSearchResult } from '../tauri/commands'
  import { openFile } from '../stores/tabs'
  import { revealLine } from '../editor/active-editor'
  import { openContextMenu } from '../context-menu/contextMenuStore'
  import Icon from './Icon.svelte'

  let inputEl = $state<HTMLInputElement | null>(null)
  let query = $state('')

  onMount(() => {
    query = $workspaceSearch.query
    void tick().then(() => inputEl?.focus())
  })

  $effect(() => {
    const root = $workspace.folderPath
    const currentQuery = query
    setWorkspaceSearchQuery(currentQuery)
    void cancelWorkspaceSearch()
    if (!root || !currentQuery.trim()) {
      clearWorkspaceSearchResults()
      return
    }
    const timer = setTimeout(() => {
      void runWorkspaceSearch(root, currentQuery)
    }, 300)
    return () => clearTimeout(timer)
  })

  async function openResult(result: WorkspaceSearchResult) {
    await openFile(result.path)
    window.setTimeout(() => revealLine(result.line_number), 0)
  }

  function onResultContextMenu(e: MouseEvent, result: WorkspaceSearchResult) {
    openContextMenu(e, {
      scope: 'search',
      path: result.path,
      workspaceRoot: $workspace.folderPath ?? undefined,
      message: result.line,
    })
  }

  function onInputKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      if ($workspaceSearch.searching) void cancelWorkspaceSearch()
      else query = ''
    }
  }
</script>

<aside class="search-panel" aria-label="Search">
  <header class="panel-header">
    <span class="panel-title">Search</span>
    {#if $workspaceSearch.searching}
      <button class="header-btn" title="Cancel Search" aria-label="Cancel Search" onclick={() => void cancelWorkspaceSearch()}>
        <Icon name="xmark" size={15} />
      </button>
    {/if}
  </header>

  <div class="search-box">
    <Icon name="search" size={15} class="search-icon" />
    <input
      bind:this={inputEl}
      bind:value={query}
      type="search"
      spellcheck="false"
      placeholder="Search workspace"
      aria-label="Search workspace"
      onkeydown={onInputKeydown}
    />
  </div>

  <div class="result-meta" role="status">
    {#if $workspaceSearch.searching}
      Searching...
    {:else if $workspaceSearch.error}
      {$workspaceSearch.error}
    {:else if $workspaceSearch.truncated}
      Showing first {$workspaceSearch.results.length} matches
    {:else if query.trim() && $workspaceSearch.results.length > 0}
      {$workspaceSearch.results.length} matches in {$workspaceSearchGroups.length} files
    {:else if query.trim()}
      No results
    {:else}
      {$workspace.folderPath ? 'Ready' : 'Open a folder'}
    {/if}
  </div>

  <div class="results" role="list">
    {#each $workspaceSearchGroups as group (group.path)}
      <section class="result-group" aria-label={group.relativePath}>
        <div class="file-row" title={group.path}>
          <Icon name="page" size={14} />
          <span class="file-name">{group.relativePath}</span>
          <span class="file-count">{group.results.length}</span>
        </div>
        {#each group.results as result (`${result.path}:${result.line_number}:${result.line}`)}
          <button
            type="button"
            class="match-row"
            title={result.line}
            oncontextmenu={(e) => onResultContextMenu(e, result)}
            onclick={() => void openResult(result)}
          >
            <span class="line-no">{result.line_number}</span>
            <span class="line-text">{result.line}</span>
          </button>
        {/each}
      </section>
    {/each}
  </div>
</aside>

<style>
  .search-panel {
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
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
  }
  .header-btn:hover {
    color: var(--foreground);
    background: var(--sidebar-accent);
  }
  .search-box {
    position: relative;
    flex-shrink: 0;
    padding: 8px 10px 6px;
  }
  .search-box :global(.search-icon) {
    position: absolute;
    left: 18px;
    top: 14px;
    color: var(--muted-foreground);
  }
  input {
    width: 100%;
    height: 28px;
    padding: 0 8px 0 30px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--background);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 12px;
    outline: none;
  }
  input:focus {
    border-color: var(--primary);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--primary) 45%, transparent);
  }
  .result-meta {
    min-height: 22px;
    flex-shrink: 0;
    padding: 0 12px 6px;
    color: var(--muted-foreground);
    font-size: 11px;
  }
  .results {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding-bottom: 8px;
  }
  .result-group {
    padding: 4px 0 6px;
  }
  .file-row {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 10px;
    color: var(--sidebar-foreground);
    font-size: 12px;
    font-weight: 600;
  }
  .file-name {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-count {
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .match-row {
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr);
    width: 100%;
    min-height: 24px;
    padding: 2px 10px;
    border: none;
    background: transparent;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .match-row:hover,
  .match-row:focus-visible {
    color: var(--foreground);
    background: var(--sidebar-accent);
    outline: none;
  }
  .line-no {
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 18px;
  }
  .line-text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 18px;
  }
</style>
