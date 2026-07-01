<script lang="ts">
  import { getCommitDiff, parseDiff, type DiffFile } from '../../tauri/commands'
  import DiffView from '../DiffView.svelte'

  // GWEN-459: renders a commit's diff via the shared DiffView (Unified/Split,
  // token colors, per-file headers). Parsing is done by the engine
  // (`parse_unified_diff`), so this component only owns the commit-hash header.
  let {
    workspacePath,
    hash,
    title,
  }: {
    workspacePath: string
    hash: string
    title: string
  } = $props()

  let files = $state<DiffFile[]>([])
  let loading = $state(true)
  let error = $state<string | null>(null)
  let requestSerial = 0

  const hasChanges = $derived(files.some((f) => f.hunks.length > 0))

  $effect(() => {
    const serial = ++requestSerial
    loading = true
    error = null
    files = []
    getCommitDiff(workspacePath, hash)
      .then(async (diff) => {
        if (serial !== requestSerial) return
        files = diff.trim() ? await parseDiff(diff) : []
      })
      .catch((e) => {
        if (serial !== requestSerial) return
        error = String(e)
      })
      .finally(() => {
        if (serial === requestSerial) loading = false
      })
  })
</script>

<div class="commit-diff-viewer">
  <header class="commit-diff-header">
    <span class="hash">{hash.slice(0, 12)}</span>
    <span class="title">{title}</span>
  </header>

  {#if loading}
    <div class="diff-info">Loading commit diff...</div>
  {:else if error}
    <div class="diff-info error">{error}</div>
  {:else if !hasChanges}
    <div class="diff-info">No changes to display.</div>
  {:else}
    <div class="commit-diff-body">
      <DiffView {files} />
    </div>
  {/if}
</div>

<style>
  .commit-diff-viewer {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background-color: var(--background);
    color: var(--foreground);
  }
  .commit-diff-header {
    min-height: 34px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    background: var(--card);
    font-family: var(--font-sans);
    font-size: 12px;
  }
  .hash {
    flex-shrink: 0;
    color: var(--primary);
    font-family: var(--font-mono);
    font-weight: 800;
  }
  .title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--foreground);
    font-weight: 600;
  }
  .diff-info {
    padding: 16px;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 13px;
  }
  .diff-info.error {
    color: var(--destructive);
  }
  /* Diff rendering now lives in the shared DiffView component. */
  .commit-diff-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
</style>
