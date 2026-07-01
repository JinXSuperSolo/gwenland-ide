import { fileIconSvg } from '../icons/gwenland-icons'

/**
 * Pure helpers for the file-attachment display component (GWEN-460): icon
 * selection, human-readable size, and graceful filename truncation. No Svelte,
 * no DOM — just data, so they're unit-testable on their own.
 *
 * Icons reuse the existing `material-icon-theme` glyphs via `fileIconSvg`
 * (which keys off a filename/extension). When only a MIME type is known — an
 * `ImageAttachment` has `mime` but no name — we map the MIME onto a
 * representative extension so the same icon set still applies, rather than
 * introducing a second icon source.
 */

/** MIME → representative extension, for attachments that carry a MIME but no
 *  filename. Falls back through broad `type/*` categories. */
export function mimeToExt(mime: string): string {
  const m = mime.toLowerCase()
  const exact: Record<string, string> = {
    'application/pdf': 'pdf',
    'application/json': 'json',
    'application/zip': 'zip',
    'application/x-yaml': 'yaml',
    'text/markdown': 'md',
    'text/html': 'html',
    'text/css': 'css',
    'text/csv': 'csv',
    'image/svg+xml': 'svg',
  }
  if (exact[m]) return exact[m]
  const [type, sub = ''] = m.split('/')
  if (type === 'image') return sub || 'png'
  if (type === 'audio') return sub === 'mpeg' ? 'mp3' : sub || 'wav'
  if (type === 'video') return sub || 'mp4'
  if (type === 'text') return 'txt'
  return ''
}

/**
 * SVG glyph string for an attachment. Prefers the filename's extension; if only
 * a MIME type is available, derives an extension from it first. Returns the raw
 * SVG (full-color material icon), so callers render it with `{@html}` — NOT via
 * `<Icon name>` (that path is for the monochrome `currentColor` glyph set).
 */
export function attachmentIconSvg(opts: { name?: string; mime?: string }): string {
  if (opts.name && opts.name.includes('.')) return fileIconSvg(opts.name)
  if (opts.mime) {
    const ext = mimeToExt(opts.mime)
    if (ext) return fileIconSvg(`f.${ext}`)
  }
  if (opts.name) return fileIconSvg(opts.name)
  return fileIconSvg('') // generic document
}

/** Bytes → human-readable size (B / KB / MB / GB). `null`/undefined → ''. */
export function formatFileSize(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return ''
  if (bytes < 1024) return `${bytes} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let value = bytes / 1024
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit++
  }
  // One decimal below 10 (e.g. 1.4 MB), whole numbers above (e.g. 24 MB).
  const rounded = value < 10 ? Math.round(value * 10) / 10 : Math.round(value)
  return `${rounded} ${units[unit]}`
}

/**
 * Middle-truncate a filename so the extension stays visible, e.g.
 * `really-long-component-name.svelte` → `really-long-…me.svelte`. Names at or
 * under `max` are returned unchanged; the extension (with its dot) is always
 * preserved, with the head trimmed and an ellipsis inserted before a short
 * tail of the base name.
 */
export function truncateFileName(name: string, max = 28): string {
  if (name.length <= max) return name
  const dot = name.lastIndexOf('.')
  // No usable extension (or a dotfile): head-truncate with a trailing ellipsis.
  if (dot <= 0) return `${name.slice(0, Math.max(1, max - 1))}…`

  const ext = name.slice(dot) // includes the dot
  const base = name.slice(0, dot)
  const ellipsis = '…'
  // Budget for the base name after reserving the extension + ellipsis.
  const budget = max - ext.length - ellipsis.length
  if (budget < 2) {
    // Extension alone is (nearly) as long as the budget — just clip the whole
    // string, keeping the tail so the extension survives.
    return `${ellipsis}${name.slice(name.length - (max - 1))}`
  }
  const tail = Math.max(2, Math.floor(budget / 3)) // keep a little of the end
  const head = budget - tail
  return `${base.slice(0, head)}${ellipsis}${base.slice(base.length - tail)}${ext}`
}
