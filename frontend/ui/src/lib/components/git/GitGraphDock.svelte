<script lang="ts">
  import type { BranchRef, CommitNode } from '../../types/git'
  import Icon from '../Icon.svelte'

  type DockMode = 'find' | 'branch' | 'commit' | 'date'

  let {
    nodes,
    branches,
    visible = true,
    onGoto,
  }: {
    nodes: CommitNode[]
    branches: BranchRef[]
    visible?: boolean
    onGoto: (node: CommitNode, source: DockMode) => void
  } = $props()

  let activeDropdown = $state<DockMode | null>(null)
  let query = $state('')
  let dateValue = $state('')
  let dockX = $state(0)
  let dockY = $state(0)
  let drag =
    $state<{
      startX: number
      startY: number
      dockX: number
      dockY: number
    } | null>(null)

  const sortedBranches = $derived.by(() =>
    [...branches].sort((a, b) => a.lane - b.lane || a.name.localeCompare(b.name)),
  )
  const normalizedQuery = $derived(query.trim().toLowerCase())
  const searchResults = $derived.by(() => {
    if (!normalizedQuery) return []
    return nodes
      .filter((node) => searchableText(node).includes(normalizedQuery))
      .slice(0, 10)
  })
  const commitList = $derived(nodes.slice(0, 120))

  function refLabel(ref: string): string {
    return ref
      .replace(/^refs\/heads\//, '')
      .replace(/^refs\/remotes\//, '')
      .replace(/^refs\/tags\//, '')
  }

  function shortMessage(message: string, length = 56): string {
    return message.length > length ? `${message.slice(0, length - 3).trimEnd()}...` : message
  }

  function branchNamesFor(node: CommitNode): string[] {
    const fromRefs = node.refs.filter((ref) => ref !== 'HEAD').map(refLabel)
    const fromBranches = branches
      .filter((branch) => branch.hash === node.hash)
      .map((branch) => refLabel(branch.name))
    return Array.from(new Set([...fromRefs, ...fromBranches]))
  }

  function searchableText(node: CommitNode): string {
    return [
      node.hash,
      node.shortHash,
      node.message,
      node.author,
      node.date,
      node.relativeDate,
      ...branchNamesFor(node),
    ]
      .join(' ')
      .toLowerCase()
  }

  function nodeForBranch(branch: BranchRef): CommitNode | null {
    return (
      nodes.find((node) => node.hash === branch.hash) ??
      nodes.find((node) => node.refs.map(refLabel).includes(refLabel(branch.name))) ??
      nodes.find((node) => node.lane === branch.lane) ??
      null
    )
  }

  function nearestNodeForDate(value: string): CommitNode | null {
    const target = Date.parse(`${value}T12:00:00`)
    if (!Number.isFinite(target)) return null
    let best: CommitNode | null = null
    let bestDelta = Number.POSITIVE_INFINITY
    for (const node of nodes) {
      const time = Date.parse(node.date)
      if (!Number.isFinite(time)) continue
      const delta = Math.abs(time - target)
      if (delta < bestDelta) {
        best = node
        bestDelta = delta
      }
    }
    return best
  }

  function gotoNode(node: CommitNode | null, source: DockMode): void {
    if (!node) return
    activeDropdown = null
    onGoto(node, source)
  }

  function gotoDate(): void {
    gotoNode(nearestNodeForDate(dateValue), 'date')
  }

  function toggleDropdown(mode: DockMode): void {
    activeDropdown = activeDropdown === mode ? null : mode
  }

  function startDrag(e: PointerEvent): void {
    drag = {
      startX: e.clientX,
      startY: e.clientY,
      dockX,
      dockY,
    }
    e.preventDefault()
  }

  function onPointerMove(e: PointerEvent): void {
    if (!drag) return
    dockX = drag.dockX + e.clientX - drag.startX
    dockY = drag.dockY + e.clientY - drag.startY
  }

  function stopDrag(): void {
    drag = null
  }
</script>

<svelte:window onpointermove={onPointerMove} onpointerup={stopDrag} />

<div
  class="git-graph-dock"
  class:hidden={!visible}
  class:dragging={!!drag}
  style={`transform: translate(calc(-50% + ${dockX}px), ${dockY}px);`}
  role="toolbar"
  aria-label="Git Graph navigation"
>
  <button
    type="button"
    class="dock-grip"
    aria-label="Move Git Graph dock"
    title="Move dock"
    onpointerdown={startDrag}
  >
    <span></span>
  </button>

  <button
    type="button"
    class:active={activeDropdown === 'find'}
    title="Find"
    aria-label="Find in Git Graph"
    aria-expanded={activeDropdown === 'find'}
    onclick={() => toggleDropdown('find')}
  >
    <Icon name="search" size={15} />
  </button>
  <button
    type="button"
    class:active={activeDropdown === 'branch'}
    title="Branch"
    aria-label="Go to branch"
    aria-expanded={activeDropdown === 'branch'}
    onclick={() => toggleDropdown('branch')}
  >
    <Icon name="git-branch" size={15} />
  </button>
  <button
    type="button"
    class:active={activeDropdown === 'commit'}
    title="Commit"
    aria-label="Go to commit"
    aria-expanded={activeDropdown === 'commit'}
    onclick={() => toggleDropdown('commit')}
  >
    <Icon name="git-commit" size={15} />
  </button>
  <button
    type="button"
    class:active={activeDropdown === 'date'}
    title="Date"
    aria-label="Go to date"
    aria-expanded={activeDropdown === 'date'}
    onclick={() => toggleDropdown('date')}
  >
    <Icon name="clock-rotate-right" size={15} />
  </button>

  {#if activeDropdown === 'find'}
    <section class="dock-popover find-popover" aria-label="Find commits">
      <input
        bind:value={query}
        type="search"
        placeholder="Search"
        aria-label="Search commits"
      />
      <div class="result-list">
        {#if normalizedQuery && searchResults.length === 0}
          <div class="empty">No matches.</div>
        {:else}
          {#each searchResults as node (node.hash)}
            <button type="button" class="result-row" onclick={() => gotoNode(node, 'find')}>
              <span class="hash">{node.shortHash}</span>
              <span class="main">{shortMessage(node.message)}</span>
              <span class="sub">{branchNamesFor(node).join(', ') || node.author}</span>
            </button>
          {/each}
        {/if}
      </div>
    </section>
  {:else if activeDropdown === 'branch'}
    <section class="dock-popover" aria-label="Branches">
      <div class="result-list">
        {#each sortedBranches as branch (`${branch.name}:${branch.hash}`)}
          <button type="button" class="result-row branch-row" onclick={() => gotoNode(nodeForBranch(branch), 'branch')}>
            <span class="lane-dot" style:--lane={`${branch.lane}`}></span>
            <span class="main">{refLabel(branch.name)}</span>
            <span class="sub">{branch.isRemote ? 'remote' : 'local'}</span>
          </button>
        {/each}
      </div>
    </section>
  {:else if activeDropdown === 'commit'}
    <section class="dock-popover" aria-label="Commits">
      <div class="result-list">
        {#each commitList as node, index (node.hash)}
          <button type="button" class="result-row" onclick={() => gotoNode(node, 'commit')}>
            <span class="hash">#{index + 1}</span>
            <span class="main">{shortMessage(node.message)}</span>
            <span class="sub">{node.shortHash} - {node.relativeDate || node.date}</span>
          </button>
        {/each}
      </div>
    </section>
  {:else if activeDropdown === 'date'}
    <section class="dock-popover date-popover" aria-label="Date">
      <input bind:value={dateValue} type="date" aria-label="Commit date" />
      <button type="button" class="go-btn" disabled={!dateValue || nodes.length === 0} onclick={gotoDate}>
        Go
      </button>
    </section>
  {/if}
</div>

<style>
  .git-graph-dock {
    position: absolute;
    left: 50%;
    bottom: 18px;
    z-index: 28;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 5px;
    border: none;
    border-radius: 7px;
    background: var(--card);
    color: var(--card-foreground);
    box-shadow:
      0 12px 30px rgba(0, 0, 0, 0.36),
      inset 0 1px 0 color-mix(in srgb, white 4%, transparent),
      inset 0 -1px 0 color-mix(in srgb, black 22%, transparent);
    opacity: 1;
    transition: opacity 0.16s ease;
    user-select: none;
  }
  .git-graph-dock::before {
    position: absolute;
    left: 9px;
    right: 9px;
    top: 0;
    height: 2px;
    border-radius: 999px;
    background: linear-gradient(
      90deg,
      #e18445 0 22%,
      #7c9eff 22% 43%,
      #6ee7b7 43% 61%,
      #f472b6 61% 78%,
      #fbbf24 78% 100%
    );
    opacity: 0.82;
    content: '';
    pointer-events: none;
  }
  .git-graph-dock.hidden {
    opacity: 0;
    pointer-events: none;
  }
  .git-graph-dock > button {
    position: relative;
    width: 29px;
    height: 27px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
    transition: background-color 0.12s ease, color 0.12s ease, transform 0.12s ease;
  }
  .git-graph-dock > button:hover,
  .git-graph-dock > button.active {
    color: var(--card-foreground);
    background: color-mix(in srgb, var(--secondary) 82%, var(--card));
  }
  .git-graph-dock > button.active::after {
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 3px;
    height: 2px;
    border-radius: 999px;
    background: var(--primary);
    content: '';
  }
  .dock-grip {
    width: 16px !important;
    cursor: grab !important;
    color: color-mix(in srgb, var(--muted-foreground) 70%, transparent) !important;
  }
  .git-graph-dock.dragging .dock-grip {
    cursor: grabbing !important;
  }
  .dock-grip span,
  .dock-grip span::before,
  .dock-grip span::after {
    display: block;
    width: 3px;
    height: 3px;
    border-radius: 999px;
    background: currentColor;
    content: '';
  }
  .dock-grip span {
    position: relative;
  }
  .dock-grip span::before,
  .dock-grip span::after {
    position: absolute;
    left: 0;
  }
  .dock-grip span::before {
    top: -6px;
  }
  .dock-grip span::after {
    top: 6px;
  }
  .dock-popover {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 9px);
    width: min(360px, calc(100vw - 48px));
    max-height: min(340px, calc(100vh - 150px));
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 7px;
    border: none;
    border-radius: 7px;
    background: var(--card);
    box-shadow:
      0 16px 34px rgba(0, 0, 0, 0.42),
      inset 0 1px 0 color-mix(in srgb, white 4%, transparent),
      inset 0 -1px 0 color-mix(in srgb, black 24%, transparent);
    color: var(--card-foreground);
  }
  .find-popover {
    width: min(420px, calc(100vw - 48px));
  }
  .date-popover {
    width: 210px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
  }
  input {
    width: 100%;
    min-width: 0;
    height: 28px;
    box-sizing: border-box;
    border: none;
    border-radius: 5px;
    background: color-mix(in srgb, var(--background) 70%, var(--card));
    box-shadow: inset 0 -1px 0 color-mix(in srgb, var(--border) 78%, transparent);
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 12px;
    padding: 0 8px;
  }
  input:focus {
    outline: none;
    box-shadow:
      inset 0 -1px 0 var(--primary),
      0 0 0 1px color-mix(in srgb, var(--primary) 16%, transparent);
  }
  .go-btn {
    height: 28px;
    padding: 0 10px;
    border: none;
    border-radius: 5px;
    background: color-mix(in srgb, var(--primary) 86%, black);
    color: var(--primary-foreground);
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }
  .go-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .result-list {
    min-height: 0;
    max-height: 284px;
    overflow: auto;
    display: grid;
    gap: 1px;
  }
  .result-row {
    position: relative;
    width: 100%;
    min-height: 30px;
    display: grid;
    grid-template-columns: 46px minmax(0, 1fr) minmax(58px, 0.48fr);
    align-items: center;
    gap: 8px;
    padding: 4px 7px 4px 9px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: var(--foreground);
    font-family: var(--font-sans);
    font-size: 11.5px;
    text-align: left;
    cursor: pointer;
  }
  .result-row::before {
    position: absolute;
    left: 0;
    top: 6px;
    bottom: 6px;
    width: 2px;
    border-radius: 999px;
    background: transparent;
    content: '';
  }
  .result-row:hover {
    background: color-mix(in srgb, var(--secondary) 76%, var(--card));
  }
  .result-row:hover::before {
    background: var(--primary);
  }
  .branch-row {
    grid-template-columns: 14px minmax(0, 1fr) 54px;
  }
  .hash {
    color: var(--primary);
    font-family: var(--font-mono);
    font-size: 10.5px;
    font-weight: 800;
  }
  .main,
  .sub {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 10.5px;
  }
  .lane-dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    background: hsl(calc(var(--lane) * 47 + 28), 72%, 62%);
  }
  .empty {
    padding: 10px 8px;
    color: var(--muted-foreground);
    font-size: 12px;
  }
  @media (max-width: 680px) {
    .git-graph-dock {
      bottom: 10px;
    }
    .result-row {
      grid-template-columns: 44px minmax(0, 1fr);
    }
    .result-row .sub {
      display: none;
    }
  }
</style>
