<script lang="ts">
  import { setPanelSize, type PanelKey } from '../stores/panels'

  /**
   * A drag handle between two panels. Two orientations:
   *
   *   - `edge="left"`  — vertical 4px strip; resizes the panel to its LEFT
   *                      (File Tree). Width grows as the pointer moves right.
   *   - `edge="top"`   — horizontal 6px strip; resizes the panel BELOW it
   *                      (Terminal). Height grows as the pointer moves up.
   *   - `edge="right"` — vertical 4px strip; resizes the panel to its RIGHT
   *                      (AI panel). Width grows as the pointer moves left.
   */
  let { target, edge }: { target: PanelKey; edge: 'left' | 'top' | 'right' } = $props()

  const vertical = $derived(edge === 'top') // a horizontal strip dragged up/down

  let dragging = $state(false)

  function onPointerDown(e: PointerEvent) {
    e.preventDefault()
    dragging = true
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return
    const handle = e.currentTarget as HTMLElement
    let size: number
    if (edge === 'left') {
      // File Tree sits to the LEFT (previous sibling): width = pointer − left edge.
      const panel = handle.previousElementSibling as HTMLElement | null
      const left = panel ? panel.getBoundingClientRect().left : 0
      size = e.clientX - left
    } else if (edge === 'right') {
      // AI panel sits to the RIGHT (next sibling): width = panel right − pointer.
      const panel = handle.nextElementSibling as HTMLElement | null
      const right = panel ? panel.getBoundingClientRect().right : window.innerWidth
      size = right - e.clientX
    } else {
      // Terminal sits BELOW (next sibling): height = panel bottom − pointer.
      const panel = handle.nextElementSibling as HTMLElement | null
      const bottom = panel ? panel.getBoundingClientRect().bottom : window.innerHeight
      size = bottom - e.clientY
    }
    // Keep the panel from swallowing the window on small screens; the store
    // additionally clamps to the panel's fixed [min, max].
    const winMax = vertical ? window.innerHeight * 0.7 : window.innerWidth * 0.6
    setPanelSize(target, Math.min(size, winMax))
  }

  function onPointerUp(e: PointerEvent) {
    dragging = false
    ;(e.target as HTMLElement).releasePointerCapture(e.pointerId)
  }
</script>

<div
  class="resize-handle"
  class:vertical
  class:dragging
  role="separator"
  aria-orientation={vertical ? 'horizontal' : 'vertical'}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
></div>

<style>
  /* Default: vertical strip (File Tree ↔ Workspace), dragged left/right.
     The strip is the (wide) hit area; a centered 1px line is the visible
     divider so panes always read as separated, not just on hover. */
  .resize-handle {
    position: relative;
    width: 5px;
    flex-shrink: 0;
    cursor: ew-resize;
    background-color: transparent;
    z-index: 5;
  }
  .resize-handle::before {
    content: '';
    position: absolute;
    inset: 0 50%;
    width: 1px;
    background-color: var(--border);
    transition: background-color 0.12s ease, box-shadow 0.12s ease;
  }
  /* Horizontal strip (Workspace ↔ Terminal), dragged up/down. */
  .resize-handle.vertical {
    width: auto;
    height: 5px;
    cursor: ns-resize;
  }
  .resize-handle.vertical::before {
    inset: 50% 0;
    width: auto;
    height: 1px;
  }
  .resize-handle:hover::before,
  .resize-handle.dragging::before {
    background-color: var(--primary);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--primary) 40%, transparent);
  }
</style>
