<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core'
  import { isImagePath, type PreviewSource } from '../stores/tabs'
  import Icon from './Icon.svelte'

  // The web-preview surface (M5). One pipeline for both source kinds: an iframe
  // rendered by the host's native OS webview (WebView2 / WebKit) — no second
  // engine, consistent with the "native WRY, single pipeline" decision in the
  // M5 spec. A dev server loads its http:// URL directly; a static file loads
  // through Tauri's asset protocol via convertFileSrc.
  //
  // Live-reload-on-save (static files) and a "server stopped" state (dev servers)
  // are tracked as separate M5 items; this pane provides the surface + manual
  // reload they hang off.
  let { source }: { source: PreviewSource } = $props()

  // What the toolbar shows vs. what the iframe loads. For a dev server they're
  // the same URL; for a static file we show the path but load the asset URL.
  const displayUrl = $derived(source.kind === 'dev-server' ? source.url : source.path)
  const frameSrc = $derived(
    source.kind === 'dev-server' ? source.url : convertFileSrc(source.path),
  )
  const isImagePreview = $derived(source.kind === 'static-file' && isImagePath(source.path))

  // Bumping this re-keys the iframe, forcing a fresh load. We can't call
  // iframe.contentWindow.location.reload() because the previewed origin
  // (localhost / asset) differs from the app's, so the DOM access is blocked.
  let reloadNonce = $state(0)
  let zoom = $state(1)
  let fit = $state(true)
  let panX = $state(0)
  let panY = $state(0)
  let dragging = $state(false)
  let dragStart = { x: 0, y: 0, panX: 0, panY: 0 }
  let imageMeta = $state<{ width: number; height: number; size: number | null } | null>(null)

  function reload() {
    reloadNonce += 1
  }

  function zoomBy(delta: number) {
    fit = false
    zoom = Math.min(6, Math.max(0.1, Math.round((zoom + delta) * 100) / 100))
  }

  function toggleFit() {
    fit = !fit
    if (fit) {
      zoom = 1
      panX = 0
      panY = 0
    }
  }

  function startPan(e: PointerEvent) {
    if (fit) return
    dragging = true
    dragStart = { x: e.clientX, y: e.clientY, panX, panY }
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }

  function movePan(e: PointerEvent) {
    if (!dragging) return
    panX = dragStart.panX + e.clientX - dragStart.x
    panY = dragStart.panY + e.clientY - dragStart.y
  }

  function stopPan(e: PointerEvent) {
    dragging = false
    ;(e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId)
  }

  async function loadImageMeta(e: Event) {
    const img = e.currentTarget as HTMLImageElement
    let size: number | null = null
    try {
      const blob = await fetch(frameSrc).then((response) => response.blob())
      size = blob.size
    } catch {
      size = null
    }
    imageMeta = { width: img.naturalWidth, height: img.naturalHeight, size }
  }

  function formatBytes(value: number | null): string {
    if (value === null) return ''
    if (value < 1024) return `${value} B`
    if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
    return `${(value / 1024 / 1024).toFixed(1)} MB`
  }
</script>

<div class="preview-pane">
  <div class="preview-toolbar">
    <Icon name="globe" size={14} />
    <span class="preview-url" title={displayUrl}>{displayUrl}</span>
    <button
      type="button"
      class="preview-action gw-transition"
      title="Reload preview"
      aria-label="Reload preview"
      onclick={reload}
    >
      <Icon name="refresh" size={14} />
    </button>
    {#if isImagePreview}
      <button
        type="button"
        class="preview-action gw-transition"
        title="Zoom out"
        aria-label="Zoom out"
        onclick={() => zoomBy(-0.1)}
      ><Icon name="minus" size={14} /></button>
      <button
        type="button"
        class="preview-action gw-transition"
        title="Zoom in"
        aria-label="Zoom in"
        onclick={() => zoomBy(0.1)}
      ><Icon name="plus" size={14} /></button>
      <button
        type="button"
        class="preview-action fit-action gw-transition"
        class:active={fit}
        title="Fit image"
        aria-label="Fit image"
        onclick={toggleFit}
      ><Icon name="collapse" size={14} /></button>
    {/if}
  </div>

  {#if isImagePreview}
    <div
      class="image-stage"
      class:pannable={!fit}
      role="application"
      aria-label="Image preview"
      onpointerdown={startPan}
      onpointermove={movePan}
      onpointerup={stopPan}
      onpointercancel={stopPan}
      onwheel={(e) => {
        if (!e.ctrlKey) return
        e.preventDefault()
        zoomBy(e.deltaY > 0 ? -0.1 : 0.1)
      }}
    >
      <img
        class="image-preview"
        class:fit={fit}
        src={frameSrc}
        alt={displayUrl}
        style:transform={fit ? undefined : `translate(${panX}px, ${panY}px) scale(${zoom})`}
        onload={loadImageMeta}
        draggable="false"
      />
      {#if imageMeta}
        <div class="image-status">
          {imageMeta.width}x{imageMeta.height}
          {#if imageMeta.size !== null}<span>{formatBytes(imageMeta.size)}</span>{/if}
          {#if !fit}<span>{Math.round(zoom * 100)}%</span>{/if}
        </div>
      {/if}
    </div>
  {:else}
    {#key `${frameSrc}::${reloadNonce}`}
      <!-- No sandbox: a local dev server / file is trusted content the user owns,
           and the preview should behave like a real browser tab (scripts, forms). -->
      <iframe class="preview-frame" src={frameSrc} title="Web preview"></iframe>
    {/key}
  {/if}
</div>

<style>
  .preview-pane {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background-color: var(--background);
  }
  .preview-toolbar {
    height: 32px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 12px;
    border-bottom: 1px solid var(--border);
    color: var(--muted-foreground);
    background-color: var(--background);
  }
  .preview-url {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--foreground);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .preview-action {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted-foreground);
    cursor: pointer;
  }
  .preview-action:hover {
    background-color: var(--secondary);
    color: var(--foreground);
  }
  .preview-action.active {
    color: var(--foreground);
    background-color: var(--secondary);
  }
  .preview-frame {
    flex: 1;
    min-height: 0;
    width: 100%;
    border: none;
    /* Web content expects an opaque (usually white) canvas, like a browser. */
    background-color: #ffffff;
  }
  .image-stage {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: auto;
    padding: 24px;
    background-color: var(--background);
    touch-action: none;
  }
  .image-stage.pannable {
    cursor: grab;
  }
  .image-stage.pannable:active {
    cursor: grabbing;
  }
  .image-preview {
    object-fit: contain;
    image-rendering: auto;
    transform-origin: center center;
    user-select: none;
  }
  .image-preview.fit {
    max-width: 100%;
    max-height: 100%;
  }
  .image-status {
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    min-height: 24px;
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 11px;
    pointer-events: none;
  }
</style>
