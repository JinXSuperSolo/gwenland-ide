<script lang="ts">
  import { getCommitDiff } from '../../tauri/commands'

  let {
    workspacePath,
    hash,
    title,
  }: {
    workspacePath: string
    hash: string
    title: string
  } = $props()

  interface Row {
    kind: 'add' | 'del' | 'ctx' | 'hunk' | 'meta'
    text: string
    oldNo: number | null
    newNo: number | null
  }

  let rows = $state<Row[]>([])
  let loading = $state(true)
  let error = $state<string | null>(null)
  let requestSerial = 0

  function parse(diff: string): Row[] {
    const out: Row[] = []
    let oldNo = 0
    let newNo = 0
    const normalized = diff.replace(/\r\n/g, '\n').replace(/\n$/, '')
    for (const line of normalized.split('\n')) {
      if (!line) continue
      if (line.startsWith('@@')) {
        const m = /@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line)
        if (m) {
          oldNo = parseInt(m[1], 10)
          newNo = parseInt(m[2], 10)
        }
        out.push({ kind: 'hunk', text: line, oldNo: null, newNo: null })
      } else if (
        line.startsWith('diff ') ||
        line.startsWith('index ') ||
        line.startsWith('--- ') ||
        line.startsWith('+++ ') ||
        line.startsWith('new file') ||
        line.startsWith('deleted file') ||
        line.startsWith('similarity ') ||
        line.startsWith('rename ')
      ) {
        out.push({ kind: 'meta', text: line, oldNo: null, newNo: null })
      } else if (line.startsWith('+')) {
        out.push({ kind: 'add', text: line.slice(1), oldNo: null, newNo: newNo++ })
      } else if (line.startsWith('-')) {
        out.push({ kind: 'del', text: line.slice(1), oldNo: oldNo++, newNo: null })
      } else if (line.startsWith('\\')) {
        out.push({ kind: 'meta', text: line, oldNo: null, newNo: null })
      } else {
        out.push({ kind: 'ctx', text: line.slice(1), oldNo: oldNo++, newNo: newNo++ })
      }
    }
    return out
  }

  $effect(() => {
    const serial = ++requestSerial
    loading = true
    error = null
    rows = []
    getCommitDiff(workspacePath, hash)
      .then((diff) => {
        if (serial !== requestSerial) return
        rows = diff.trim() ? parse(diff) : []
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
  {:else if rows.length === 0}
    <div class="diff-info">No changes to display.</div>
  {:else}
    <div class="diff-grid" role="table" aria-label="Commit diff">
      {#each rows as row, index (index)}
        <div class="diff-row {row.kind}" role="row">
          <span class="ln old">{row.oldNo ?? ''}</span>
          <span class="ln new">{row.newNo ?? ''}</span>
          <span class="sign">
            {row.kind === 'add' ? '+' : row.kind === 'del' ? '-' : ''}
          </span>
          <span class="code">{row.text}</span>
        </div>
      {/each}
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
  .diff-grid {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: table;
    width: 100%;
    border-collapse: collapse;
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .diff-row {
    display: table-row;
    white-space: pre;
  }
  .diff-row > span {
    display: table-cell;
    vertical-align: top;
  }
  .ln {
    width: 1%;
    min-width: 38px;
    padding: 0 8px;
    text-align: right;
    color: var(--muted-foreground);
    opacity: 0.6;
    user-select: none;
    border-right: 1px solid var(--border);
  }
  .sign {
    width: 14px;
    text-align: center;
    user-select: none;
    color: var(--muted-foreground);
  }
  .code {
    width: 100%;
    padding-right: 16px;
  }
  .diff-row.add {
    background-color: rgba(40, 167, 69, 0.14);
  }
  .diff-row.add .sign {
    color: #5fb572;
  }
  .diff-row.del {
    background-color: rgba(220, 53, 69, 0.14);
  }
  .diff-row.del .sign {
    color: #e0707c;
  }
  .diff-row.hunk {
    background-color: var(--secondary);
    color: var(--primary);
  }
  .diff-row.hunk .code {
    color: var(--primary);
  }
  .diff-row.meta {
    color: var(--muted-foreground);
    opacity: 0.7;
  }
</style>
