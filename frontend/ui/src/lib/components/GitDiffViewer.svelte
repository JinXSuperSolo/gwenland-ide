<script lang="ts">
  import { gitDiffFile } from '../tauri/commands'

  // GWEN-330: read-only unified-diff viewer for one file. Rendered in a normal
  // editor tab; closing it touches no git state. Added lines green, removed red,
  // with old/new line numbers in the gutter.
  let { root, path, untracked }: { root: string; path: string; untracked: boolean } = $props()

  interface Row {
    kind: 'add' | 'del' | 'ctx' | 'hunk' | 'meta'
    text: string
    oldNo: number | null
    newNo: number | null
  }

  let rows = $state<Row[]>([])
  let loading = $state(true)
  let error = $state<string | null>(null)

  function parse(diff: string): Row[] {
    const out: Row[] = []
    let oldNo = 0
    let newNo = 0
    for (const line of diff.split('\n')) {
      if (line.startsWith('@@')) {
        // @@ -oldStart,oldCount +newStart,newCount @@
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
        // "\ No newline at end of file"
        out.push({ kind: 'meta', text: line, oldNo: null, newNo: null })
      } else {
        out.push({ kind: 'ctx', text: line.slice(1), oldNo: oldNo++, newNo: newNo++ })
      }
    }
    return out
  }

  $effect(() => {
    loading = true
    error = null
    gitDiffFile(root, path, untracked)
      .then((diff) => {
        rows = diff.trim() ? parse(diff) : []
      })
      .catch((e) => (error = String(e)))
      .finally(() => (loading = false))
  })
</script>

<div class="diff-viewer">
  {#if loading}
    <div class="diff-info">Loading diff…</div>
  {:else if error}
    <div class="diff-info error">{error}</div>
  {:else if rows.length === 0}
    <div class="diff-info">No changes to display.</div>
  {:else}
    <div class="diff-grid" role="table">
      {#each rows as row}
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
  .diff-viewer {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background-color: var(--background);
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.5;
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
    display: table;
    width: 100%;
    border-collapse: collapse;
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
    padding-right: 16px;
    width: 100%;
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
