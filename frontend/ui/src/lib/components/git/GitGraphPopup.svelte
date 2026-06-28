<script lang="ts">
  import {
    commitDetailsCache,
    commitDetailsKey,
    loadCommitDetails,
  } from '../../stores/gitGraph'
  import { openCommitDiff } from '../../stores/tabs'
  import type { CommitNode } from '../../types/git'
  import Icon from '../Icon.svelte'

  let {
    node,
    workspacePath,
    x,
    y,
    onClose,
  }: {
    node: CommitNode
    workspacePath: string
    x: number
    y: number
    onClose: () => void
  } = $props()

  const refs = $derived(node.refs.filter((ref) => ref !== 'HEAD'))
  const detailKey = $derived(commitDetailsKey(workspacePath, node.hash))
  const detailState = $derived(
    $commitDetailsCache[detailKey] ?? { value: null, loading: false, error: null },
  )
  const details = $derived(detailState.value)
  const message = $derived(details?.fullMessage.trim() || node.message)

  $effect(() => {
    if (!workspacePath || !node.hash) return
    void loadCommitDetails(workspacePath, node.hash)
  })

  function openFullView() {
    openCommitDiff(workspacePath, node.hash, node.shortHash, node.message)
    onClose()
  }
</script>

<section
  class="git-graph-popup"
  style:left={`${x}px`}
  style:top={`${y}px`}
  aria-label="Commit details"
>
  <header>
    <div class="title">
      <span class="hash">{node.shortHash}</span>
      {#if node.isMerge}<span class="badge">merge</span>{/if}
      {#if node.isHead}<span class="badge head">HEAD</span>{/if}
    </div>
    <button type="button" class="close" aria-label="Close commit popup" onclick={onClose}>
      <Icon name="xmark" size={13} />
    </button>
  </header>

  <div class="popup-body">
    <div class="message">{message}</div>

    <dl>
      <div>
        <dt>Hash</dt>
        <dd class="mono">{node.hash}</dd>
      </div>
      <div>
        <dt>Author</dt>
        <dd>{details?.author ?? node.author}</dd>
      </div>
      <div>
        <dt>Date</dt>
        <dd>{details?.date ?? node.date}</dd>
      </div>
      {#if refs.length}
        <div>
          <dt>Refs</dt>
          <dd class="refs">{refs.join(', ')}</dd>
        </div>
      {/if}
    </dl>

    {#if detailState.loading}
      <div class="detail-note">Loading commit details...</div>
    {:else if detailState.error}
      <div class="detail-note error">{detailState.error}</div>
    {:else if details}
      <div class="stats" aria-label="Commit change statistics">
        <span>{details.filesChanged.length} files</span>
        <span class="added">+{details.insertions}</span>
        <span class="removed">-{details.deletions}</span>
      </div>

      {#if details.filesChanged.length}
        <div class="files" role="list" aria-label="Files changed">
          {#each details.filesChanged as file (file.status + ':' + file.path)}
            <div class="file-row" role="listitem">
              <span class="file-status" data-status={file.status}>{file.status}</span>
              <span class="file-path">{file.path}</span>
            </div>
          {/each}
        </div>
      {:else}
        <div class="detail-note">No files changed.</div>
      {/if}
    {/if}
  </div>

  <footer>
    <button type="button" class="full-view" onclick={openFullView}>
      <Icon name="open-in-window" size={14} />
      <span>Full View</span>
    </button>
  </footer>
</section>

<style>
  .git-graph-popup {
    position: absolute;
    z-index: 35;
    width: min(380px, calc(100% - 28px));
    max-width: 380px;
    max-height: min(440px, calc(100% - 28px));
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid color-mix(in srgb, var(--primary) 28%, var(--border));
    border-radius: 7px;
    background: color-mix(in srgb, var(--popover) 96%, black);
    box-shadow: var(--shadow-lg);
    color: var(--popover-foreground);
    font-family: var(--font-sans);
  }
  header,
  footer {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 10px;
  }
  header {
    padding-bottom: 7px;
  }
  footer {
    justify-content: flex-end;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
  .popup-body {
    min-height: 0;
    overflow: auto;
    padding: 0 10px 10px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }
  .hash {
    color: var(--primary);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 800;
  }
  .badge {
    padding: 1px 5px;
    border-radius: 4px;
    background: color-mix(in srgb, #7c9eff 18%, transparent);
    color: #9cb5ff;
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
  }
  .badge.head {
    background: color-mix(in srgb, var(--primary) 18%, transparent);
    color: var(--primary);
  }
  .close,
  .full-view {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 5px;
    font-family: var(--font-sans);
    cursor: pointer;
  }
  .close {
    width: 22px;
    height: 22px;
    background: transparent;
    color: var(--muted-foreground);
  }
  .close:hover {
    color: var(--foreground);
    background: var(--secondary);
  }
  .message {
    color: var(--foreground);
    font-size: 12.5px;
    font-weight: 600;
    line-height: 1.35;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  dl {
    display: grid;
    gap: 6px;
    margin: 10px 0 0;
  }
  dl div {
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr);
    gap: 8px;
  }
  dt,
  dd {
    margin: 0;
    font-size: 11px;
    line-height: 1.35;
  }
  dt {
    color: var(--muted-foreground);
  }
  dd {
    min-width: 0;
    color: var(--foreground);
    overflow-wrap: anywhere;
  }
  .mono,
  .refs {
    font-family: var(--font-mono);
    font-size: 10.5px;
  }
  .refs {
    color: #7c9eff;
  }
  .detail-note {
    margin-top: 10px;
    color: var(--muted-foreground);
    font-size: 11px;
  }
  .detail-note.error {
    color: var(--destructive);
    overflow-wrap: anywhere;
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .stats span {
    padding: 2px 6px;
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--muted-foreground);
  }
  .stats .added {
    color: #5fb572;
  }
  .stats .removed {
    color: #e0707c;
  }
  .files {
    display: grid;
    gap: 3px;
    margin-top: 8px;
    max-height: 150px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: color-mix(in srgb, var(--background) 60%, transparent);
    padding: 5px;
  }
  .file-row {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    align-items: center;
    gap: 6px;
    min-height: 20px;
    font-size: 11px;
  }
  .file-status {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 16px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--primary) 14%, transparent);
    color: var(--primary);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 800;
  }
  .file-status[data-status='A'],
  .file-status[data-status='C'] {
    color: #5fb572;
    background: rgba(40, 167, 69, 0.14);
  }
  .file-status[data-status='D'] {
    color: #e0707c;
    background: rgba(220, 53, 69, 0.14);
  }
  .file-status[data-status='R'] {
    color: #9cb5ff;
    background: color-mix(in srgb, #7c9eff 16%, transparent);
  }
  .file-path {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--foreground);
    font-family: var(--font-mono);
  }
  .full-view {
    gap: 6px;
    height: 27px;
    padding: 0 10px;
    background: var(--primary);
    color: var(--primary-foreground);
    font-size: 12px;
    font-weight: 700;
  }
  .full-view:hover {
    filter: brightness(1.06);
  }
</style>
