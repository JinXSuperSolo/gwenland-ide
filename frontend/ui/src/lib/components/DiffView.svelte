<script lang="ts">
  import type { DiffFile } from '../tauri/commands'
  import {
    unifiedRows,
    splitRows,
    fileStats,
    fileDisplayPath,
    fileNameParts,
  } from '../ai/diff-rows'
  import { editorPreferences, setDiffViewMode } from '../stores/editor-preferences'
  import Icon from './Icon.svelte'

  /**
   * Reusable structured-diff renderer (GWEN-459) — the single source of truth
   * for displaying a parsed diff. Takes engine `DiffFile[]` (from
   * `parse_unified_diff`) and renders each file with a header (filename +
   * extension badge + +N/−N stats) and a Unified ↔ Split layout toggle. The
   * chosen layout is shared and persisted via `editorPreferences.diffViewMode`.
   *
   * Colors come entirely from `--diff-*` tokens (tokens.css), never hardcoded,
   * so this reads correctly on any dark surface. Line numbers in split mode
   * track old/new columns independently (see `diff-rows.ts`).
   */
  let { files, showToggle = true }: { files: DiffFile[]; showToggle?: boolean } = $props()

  const mode = $derived($editorPreferences.diffViewMode)
</script>

<div class="dv">
  {#if showToggle}
    <div class="dv-toolbar">
      <div class="dv-seg" role="group" aria-label="Diff view mode">
        <button
          type="button"
          class="dv-seg-btn"
          class:active={mode === 'unified'}
          aria-pressed={mode === 'unified'}
          onclick={() => setDiffViewMode('unified')}
        >
          Unified
        </button>
        <button
          type="button"
          class="dv-seg-btn"
          class:active={mode === 'split'}
          aria-pressed={mode === 'split'}
          onclick={() => setDiffViewMode('split')}
        >
          Split
        </button>
      </div>
    </div>
  {/if}

  {#each files as file, fi (fi)}
    {@const stats = fileStats(file)}
    {@const path = fileDisplayPath(file)}
    {@const parts = fileNameParts(path)}
    <section class="dv-file">
      <header class="dv-file-head" title={path}>
        <Icon name="page" size={13} />
        <span class="dv-file-name">{parts.name}</span>
        {#if parts.ext}<span class="dv-ext">{parts.ext}</span>{/if}
        <span class="dv-stat add">+{stats.added}</span>
        <span class="dv-stat del">−{stats.removed}</span>
      </header>

      {#if mode === 'split'}
        <div class="dv-grid split" role="table">
          {#each splitRows(file) as row}
            {#if row.kind === 'hunk'}
              <div class="dv-row hunk" role="row">
                <span class="dv-hunk-text" role="cell">{row.header}</span>
              </div>
            {:else}
              <div class="dv-row pair" role="row">
                <span class="dv-num" role="cell">{row.old?.lineNo ?? ''}</span>
                <span class="dv-cell {row.old?.kind ?? 'blank'}" role="cell">{row.old?.text ?? ''}</span>
                <span class="dv-num" role="cell">{row.new?.lineNo ?? ''}</span>
                <span class="dv-cell {row.new?.kind ?? 'blank'}" role="cell">{row.new?.text ?? ''}</span>
              </div>
            {/if}
          {/each}
        </div>
      {:else}
        <div class="dv-grid unified" role="table">
          {#each unifiedRows(file) as row}
            {#if row.kind === 'hunk'}
              <div class="dv-row hunk" role="row">
                <span class="dv-hunk-text" role="cell">{row.text}</span>
              </div>
            {:else}
              <div class="dv-row {row.kind}" role="row">
                <span class="dv-num" role="cell">{row.oldNo ?? ''}</span>
                <span class="dv-num" role="cell">{row.newNo ?? ''}</span>
                <span class="dv-sign" role="cell"
                  >{row.kind === 'add' ? '+' : row.kind === 'del' ? '−' : ''}</span
                >
                <span class="dv-code" role="cell">{row.text}</span>
              </div>
            {/if}
          {/each}
        </div>
      {/if}
    </section>
  {/each}
</div>

<style>
  .dv {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .dv-toolbar {
    display: flex;
    justify-content: flex-end;
    padding: 6px 10px;
    flex-shrink: 0;
  }
  /* Borderless segmented toggle, matching the M26 ghost-pill style. */
  .dv-seg {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: 999px;
    background-color: color-mix(in srgb, var(--muted-foreground) 12%, transparent);
  }
  .dv-seg-btn {
    padding: 3px 12px;
    border: none;
    border-radius: 999px;
    background: transparent;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: color 0.12s ease, background-color 0.12s ease;
  }
  .dv-seg-btn:hover {
    color: var(--foreground);
  }
  .dv-seg-btn.active {
    color: var(--primary-foreground);
    background-color: var(--primary);
  }

  .dv-file {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .dv-file-head {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 12px;
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--muted-foreground);
    background-color: var(--diff-hunk-bg);
    position: sticky;
    top: 0;
    z-index: 1;
  }
  .dv-file-name {
    color: var(--foreground);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dv-ext {
    flex-shrink: 0;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background-color: color-mix(in srgb, var(--primary) 16%, transparent);
    color: var(--primary);
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
  }
  .dv-stat {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .dv-stat.add {
    margin-left: auto;
    color: var(--diff-add-text);
  }
  .dv-stat.del {
    color: var(--diff-del-text);
  }

  .dv-grid {
    display: table;
    width: 100%;
    border-collapse: collapse;
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.5;
    overflow-x: auto;
  }
  .dv-row {
    display: table-row;
    white-space: pre;
  }
  .dv-row > span {
    display: table-cell;
    vertical-align: top;
  }

  .dv-num {
    width: 1%;
    min-width: 40px;
    padding: 0 8px;
    text-align: right;
    color: var(--diff-num);
    background-color: var(--diff-num-bg);
    user-select: none;
    border-right: 1px solid var(--diff-divider);
  }

  /* Unified layout: [oldNo][newNo][sign][code]. */
  .dv-sign {
    width: 14px;
    text-align: center;
    user-select: none;
    color: var(--muted-foreground);
  }
  .dv-code {
    padding: 0 16px 0 6px;
    width: 100%;
  }
  .dv-row.add {
    background-color: var(--diff-add-line);
  }
  .dv-row.add .dv-sign {
    color: var(--diff-add-text);
  }
  .dv-row.del {
    background-color: var(--diff-del-line);
  }
  .dv-row.del .dv-sign {
    color: var(--diff-del-text);
  }

  /* Split layout: [oldNo][oldCell][newNo][newCell], center divider on the 3rd. */
  .dv-grid.split .dv-num:nth-of-type(3) {
    border-left: 2px solid var(--diff-divider);
  }
  .dv-cell {
    width: 50%;
    padding: 0 12px;
  }
  .dv-cell.add {
    background-color: var(--diff-add-line);
    box-shadow: inset 3px 0 0 var(--diff-add-gutter);
  }
  .dv-cell.del {
    background-color: var(--diff-del-line);
    box-shadow: inset 3px 0 0 var(--diff-del-gutter);
  }
  .dv-cell.blank {
    background-color: rgba(255, 255, 255, 0.012);
  }

  /* Hunk header row spans the full width in both layouts. Using `display:block`
     (not table-row) lets its single child fill the width without needing a
     colspan, which CSS tables don't offer. */
  .dv-row.hunk {
    display: block;
    background-color: var(--diff-hunk-bg);
  }
  .dv-hunk-text {
    display: block;
    padding: 1px 12px;
    color: var(--primary);
    user-select: none;
  }
</style>
