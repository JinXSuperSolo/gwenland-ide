<script lang="ts" module>
  /**
   * Normalized input for the attachment chip (GWEN-460). Callers map their
   * existing shape onto this:
   *  - a user file ref → `{ kind: 'path', name, path, size? }`
   *  - a pasted/picked/AI image or generated file with bytes in hand →
   *    `{ kind: 'data', name, mime, base64, size? }`
   * The two kinds drive the action: `path` opens in the editor (the file is
   * already on disk in the project), `data` saves the bytes to disk.
   */
  export type FileAttachmentInput =
    | { kind: 'path'; name: string; path: string; mime?: string; size?: number }
    | { kind: 'data'; name: string; mime: string; base64: string; size?: number }
</script>

<script lang="ts">
  import { openFile } from '../stores/tabs'
  import { attachmentIconSvg, formatFileSize, truncateFileName } from '../ai/file-attachment'
  import Icon from './Icon.svelte'

  let {
    attachment,
    variant = 'sm',
  }: {
    attachment: FileAttachmentInput
    /** `sm` = inline within a chat message; `card` = standalone attachment card. */
    variant?: 'sm' | 'card'
  } = $props()

  const iconSvg = $derived(
    attachmentIconSvg({ name: attachment.name, mime: attachment.mime })
  )
  const displayName = $derived(truncateFileName(attachment.name, variant === 'card' ? 40 : 26))
  const sizeLabel = $derived(formatFileSize(attachment.size))
  const canDownload = $derived(attachment.kind === 'data')
  const actionLabel = $derived(canDownload ? 'Download' : 'Open in editor')

  /** Decode base64 → Blob without a data: URL round-trip. */
  function base64ToBlob(base64: string, mime: string): Blob {
    const bin = atob(base64)
    const bytes = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
    return new Blob([bytes], { type: mime || 'application/octet-stream' })
  }

  function onAction() {
    if (attachment.kind === 'data') {
      const url = URL.createObjectURL(base64ToBlob(attachment.base64, attachment.mime))
      const a = document.createElement('a')
      a.href = url
      a.download = attachment.name || 'download'
      document.body.appendChild(a)
      a.click()
      a.remove()
      // Revoke on the next tick so the click has consumed the URL.
      setTimeout(() => URL.revokeObjectURL(url), 0)
    } else {
      void openFile(attachment.path, { preview: false })
    }
  }
</script>

<div class="fa {variant}" title={attachment.name}>
  <!-- eslint-disable-next-line svelte/no-at-html-tags — trusted local icon SVG -->
  <span class="fa-icon">{@html iconSvg}</span>
  <span class="fa-meta">
    <span class="fa-name">{displayName}</span>
    {#if sizeLabel}<span class="fa-size">{sizeLabel}</span>{/if}
  </span>
  <button
    type="button"
    class="fa-action"
    title={actionLabel}
    aria-label={`${actionLabel}: ${attachment.name}`}
    onclick={onAction}
  >
    <Icon name={canDownload ? 'download' : 'open-in-window'} size={variant === 'card' ? 15 : 13} />
  </button>
</div>

<style>
  /* Muted-outline chrome, matching the AI panel's message surfaces
     (--ai-bg-surface fill + subtle border), consistent with AiMessage.svelte. */
  .fa {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    max-width: 100%;
    background-color: var(--ai-bg-surface, var(--card));
    border: 1px solid var(--ai-border-subtle, var(--border));
    color: var(--ai-text-primary, var(--foreground));
    box-sizing: border-box;
  }
  .fa.sm {
    padding: 4px 6px 4px 8px;
    border-radius: 10px;
    max-width: 260px;
  }
  .fa.card {
    width: 100%;
    padding: 10px 10px 10px 12px;
    border-radius: 12px;
  }

  .fa-icon {
    display: inline-flex;
    flex-shrink: 0;
    line-height: 0;
  }
  .fa.sm .fa-icon :global(svg) {
    width: 16px;
    height: 16px;
  }
  .fa.card .fa-icon :global(svg) {
    width: 26px;
    height: 26px;
  }

  .fa-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
    gap: 1px;
  }
  .fa-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-sans);
    font-weight: 600;
  }
  .fa.sm .fa-name {
    font-size: 11.5px;
  }
  .fa.card .fa-name {
    font-size: 12.5px;
  }
  .fa-size {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--ai-text-muted, var(--muted-foreground));
    font-variant-numeric: tabular-nums;
  }

  .fa-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: var(--ai-text-muted, var(--muted-foreground));
    cursor: pointer;
    transition: color 0.12s ease, background-color 0.12s ease;
  }
  .fa.sm .fa-action {
    width: 22px;
    height: 22px;
  }
  .fa.card .fa-action {
    width: 28px;
    height: 28px;
  }
  .fa-action:hover {
    color: var(--ai-text-primary, var(--foreground));
    background-color: var(--ai-bg-hover, var(--secondary));
  }
</style>
