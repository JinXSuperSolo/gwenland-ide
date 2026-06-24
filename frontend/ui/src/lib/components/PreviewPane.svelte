<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core'
  import type { PreviewSource } from '../stores/tabs'
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

  // Bumping this re-keys the iframe, forcing a fresh load. We can't call
  // iframe.contentWindow.location.reload() because the previewed origin
  // (localhost / asset) differs from the app's, so the DOM access is blocked.
  let reloadNonce = $state(0)
  function reload() {
    reloadNonce += 1
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
  </div>

  {#key `${frameSrc}::${reloadNonce}`}
    <!-- No sandbox: a local dev server / file is trusted content the user owns,
         and the preview should behave like a real browser tab (scripts, forms). -->
    <iframe class="preview-frame" src={frameSrc} title="Web preview"></iframe>
  {/key}
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
  .preview-frame {
    flex: 1;
    min-height: 0;
    width: 100%;
    border: none;
    /* Web content expects an opaque (usually white) canvas, like a browser. */
    background-color: #ffffff;
  }
</style>
