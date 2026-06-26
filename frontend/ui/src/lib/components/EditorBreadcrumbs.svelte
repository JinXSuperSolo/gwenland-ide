<script lang="ts">
  import { expandPanel } from '../stores/panels'
  import { requestTreeReveal } from '../stores/file-tree'
  import { openFile } from '../stores/tabs'
  import { workspace } from '../stores/workspace'

  let {
    path,
    groupId,
  }: {
    path: string
    groupId?: string
  } = $props()

  interface Segment {
    label: string
    path: string
    file: boolean
  }

  function sep(value: string): string {
    return value.includes('\\') ? '\\' : '/'
  }

  function basename(value: string): string {
    return value.split(/[\\/]/).filter(Boolean).pop() || value
  }

  function join(parent: string, child: string): string {
    const s = sep(parent)
    return parent.endsWith(s) ? parent + child : parent + s + child
  }

  function norm(value: string): string {
    return value.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()
  }

  const segments = $derived.by<Segment[]>(() => {
    if (!path) return []
    const root = $workspace.folderPath
    if (root && (norm(path) === norm(root) || norm(path).startsWith(norm(root) + '/'))) {
      const relative = norm(path) === norm(root) ? '' : path.slice(root.length).replace(/^[\\/]+/, '')
      const parts = relative.split(/[\\/]/).filter(Boolean)
      let current = root
      const next: Segment[] = [{ label: basename(root), path: root, file: parts.length === 0 }]
      parts.forEach((part, index) => {
        current = join(current, part)
        next.push({ label: part, path: current, file: index === parts.length - 1 })
      })
      return next
    }

    const parts = path.split(/[\\/]/).filter(Boolean)
    let current = /^[a-zA-Z]:/.test(path) ? parts.shift() ?? path : ''
    const next: Segment[] = []
    for (const [index, part] of parts.entries()) {
      current = current ? join(current, part) : part
      next.push({ label: part, path: current, file: index === parts.length - 1 })
    }
    return next.length ? next : [{ label: path, path, file: true }]
  })

  function openSegment(segment: Segment): void {
    expandPanel('fileTree')
    requestTreeReveal(segment.path)
    if (segment.file) void openFile(segment.path, { groupId })
  }
</script>

{#if segments.length}
  <nav class="breadcrumbs" aria-label="Editor Breadcrumbs">
    {#each segments as segment, index (segment.path + index)}
      {#if index > 0}<span class="crumb-sep">/</span>{/if}
      <button
        type="button"
        class:file={segment.file}
        title={segment.path}
        onclick={() => openSegment(segment)}
      >
        {segment.label}
      </button>
    {/each}
  </nav>
{/if}

<style>
  .breadcrumbs {
    height: 26px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--background) 92%, var(--card));
    overflow: hidden;
    white-space: nowrap;
  }
  .breadcrumbs button {
    min-width: 0;
    max-width: 180px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 12px;
    padding: 3px 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }
  .breadcrumbs button:hover,
  .breadcrumbs button.file {
    color: var(--foreground);
  }
  .breadcrumbs button:hover {
    background: var(--secondary);
  }
  .crumb-sep {
    color: var(--muted-foreground);
    opacity: 0.65;
    font-size: 11px;
  }
</style>
