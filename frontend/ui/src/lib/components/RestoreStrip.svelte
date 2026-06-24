<script lang="ts">
  import { expandPanel, type PanelKey } from '../stores/panels'
  import { fade } from 'svelte/transition'

  /**
   * A thin clickable strip shown where a collapsed panel used to be.
   *   - orientation="vertical"   — for File Tree (collapses left/right).
   *   - orientation="horizontal" — for Terminal (collapses up/down); strip
   *                                spans full width at the bottom.
   */
  let {
    target,
    label,
    orientation = 'vertical',
  }: {
    target: PanelKey
    label: string
    orientation?: 'vertical' | 'horizontal'
  } = $props()

  const horizontal = $derived(orientation === 'horizontal')
</script>

<button
  class="restore-strip gw-transition"
  class:horizontal
  in:fade={{ duration: 120 }}
  title={`Show ${label}`}
  aria-label={`Show ${label}`}
  onclick={() => expandPanel(target)}
>
  <span class="grip"></span>
</button>

<style>
  .restore-strip {
    flex-shrink: 0;
    background-color: var(--secondary);
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: background-color 0.12s ease;
  }
  /* Vertical (File Tree): thin column, grip is a vertical bar. */
  .restore-strip:not(.horizontal) {
    width: 8px;
    border-left: 1px solid var(--border);
    border-right: 1px solid var(--border);
  }
  .restore-strip:not(.horizontal) .grip {
    width: 2px;
    height: 28px;
  }
  /* Horizontal (Terminal): thin full-width row, grip is a horizontal bar. */
  .restore-strip.horizontal {
    height: 8px;
    width: 100%;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }
  .restore-strip.horizontal .grip {
    height: 2px;
    width: 28px;
  }
  .restore-strip:hover {
    background-color: var(--sidebar-accent);
  }
  .grip {
    border-radius: 2px;
    background-color: var(--muted-foreground);
    opacity: 0.6;
  }
  .restore-strip:hover .grip {
    background-color: var(--primary);
    opacity: 1;
  }
</style>
