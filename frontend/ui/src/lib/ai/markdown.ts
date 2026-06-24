/**
 * Tiny, dependency-free Markdown renderer for AI message prose (Milestone 8).
 *
 * Fenced code blocks are handled upstream by `parseSegments`, so this only sees
 * the prose between them. It covers what assistant replies actually use:
 * headings, bold/italic, inline code, bullet/ordered lists, blockquotes, GFM
 * tables, LaTeX math (via `./math`), and paragraphs. Security: the source is
 * HTML-escaped FIRST, then formatting is applied to the escaped text, and links
 * are restricted to http(s)/mailto — safe to render with `{@html}`.
 */

import { renderMath } from './math'

// Private-use sentinel wrapping a stash index; computed (not a literal) so the
// source stays free of invisible characters.
const SENT = String.fromCharCode(0xe000)
const RESTORE = new RegExp(`${SENT}(\\d+)${SENT}`, 'g')

function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

/**
 * Apply inline formatting. Math and inline code are rendered first and stashed
 * behind the sentinel so the escape/bold/italic/link passes can't mangle their
 * contents; the placeholders are restored at the end.
 */
function inline(raw: string): string {
  const stash: string[] = []
  const keep = (html: string) => `${SENT}${stash.push(html) - 1}${SENT}`

  let s = raw
  // Math: $$display$$, \[display\], \(inline\); then single-$ only when it looks
  // like math (so prices like "$5 … $10" aren't swallowed).
  s = s.replace(/\$\$([^$]+?)\$\$/g, (_m, t) => keep(renderMath(t, true)))
  s = s.replace(/\\\[([\s\S]+?)\\\]/g, (_m, t) => keep(renderMath(t, true)))
  s = s.replace(/\\\(([\s\S]+?)\\\)/g, (_m, t) => keep(renderMath(t, false)))
  s = s.replace(/\$([^$\n]+?)\$/g, (m, t) => (/[\\^_{}]/.test(t) ? keep(renderMath(t, false)) : m))
  // Inline code (stashed so its contents stay literal).
  s = s.replace(/`([^`]+)`/g, (_m, c) => keep(`<code class="md-code">${escapeHtml(c)}</code>`))

  s = escapeHtml(s)
  s = s.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
  s = s.replace(/(^|[^*])\*([^*\n]+)\*/g, '$1<em>$2</em>')
  s = s.replace(/(^|[^_\w])_([^_\n]+)_/g, '$1<em>$2</em>')
  s = s.replace(
    /\[([^\]]+)\]\((https?:\/\/[^)\s]+|mailto:[^)\s]+)\)/g,
    (_m, text, url) => `<a href="${url}" target="_blank" rel="noopener noreferrer">${text}</a>`
  )

  return s.replace(RESTORE, (_m, idx) => stash[Number(idx)])
}

const HEADING = /^(#{1,6})\s+(.*)$/
const UL_ITEM = /^[-*+]\s+(.*)$/
const OL_ITEM = /^(\d+)[.)]\s+(.*)$/
const QUOTE = /^>\s?(.*)$/

type Align = 'left' | 'center' | 'right' | ''

/** Split a `| a | b |` table row into trimmed cells (outer pipes dropped). */
function splitRow(line: string): string[] {
  let s = line.trim()
  if (s.startsWith('|')) s = s.slice(1)
  if (s.endsWith('|')) s = s.slice(0, -1)
  return s.split('|').map((c) => c.trim())
}

/** True for a GFM separator row like `| --- | :--: | ---: |`. */
function isTableSeparator(line: string): boolean {
  const s = line.trim()
  if (!s.includes('-') || !s.includes('|')) return false
  return /^\|?\s*:?-{1,}:?\s*(\|\s*:?-{1,}:?\s*)*\|?$/.test(s)
}

function parseAligns(line: string): Align[] {
  return splitRow(line).map((c) => {
    const l = c.startsWith(':')
    const r = c.endsWith(':')
    if (l && r) return 'center'
    if (r) return 'right'
    if (l) return 'left'
    return ''
  })
}

function alignAttr(a: Align | undefined): string {
  return a ? ` style="text-align:${a}"` : ''
}

function renderTable(header: string[], aligns: Align[], rows: string[][]): string {
  const th = header.map((c, i) => `<th${alignAttr(aligns[i])}>${inline(c)}</th>`).join('')
  const body = rows
    .map((r) => `<tr>${r.map((c, i) => `<td${alignAttr(aligns[i])}>${inline(c)}</td>`).join('')}</tr>`)
    .join('')
  return `<table class="md-table"><thead><tr>${th}</tr></thead><tbody>${body}</tbody></table>`
}

/** Render Markdown prose to safe HTML. */
export function renderMarkdown(src: string): string {
  const lines = src.split('\n')
  const out: string[] = []
  let list: 'ul' | 'ol' | null = null

  const closeList = () => {
    if (list) {
      out.push(`</${list}>`)
      list = null
    }
  }

  let i = 0
  while (i < lines.length) {
    const line = lines[i]
    const trimmed = line.trim()

    if (trimmed === '') {
      closeList()
      i++
      continue
    }

    // Display math block: $$ … $$ (single or multi-line).
    if (trimmed.startsWith('$$')) {
      closeList()
      const oneLine = /^\$\$(.+?)\$\$$/.exec(trimmed)
      if (oneLine) {
        out.push(renderMath(oneLine[1], true))
        i++
        continue
      }
      const buf = [trimmed.slice(2)]
      i++
      while (i < lines.length && !lines[i].includes('$$')) {
        buf.push(lines[i])
        i++
      }
      if (i < lines.length) {
        buf.push(lines[i].slice(0, lines[i].indexOf('$$')))
        i++
      }
      out.push(renderMath(buf.join('\n'), true))
      continue
    }

    const h = HEADING.exec(trimmed)
    if (h) {
      closeList()
      const level = h[1].length
      out.push(`<h${level} class="md-h md-h${level}">${inline(h[2])}</h${level}>`)
      i++
      continue
    }

    const ul = UL_ITEM.exec(trimmed)
    if (ul) {
      if (list !== 'ul') {
        closeList()
        out.push('<ul class="md-ul">')
        list = 'ul'
      }
      out.push(`<li>${inline(ul[1])}</li>`)
      i++
      continue
    }

    const ol = OL_ITEM.exec(trimmed)
    if (ol) {
      if (list !== 'ol') {
        closeList()
        out.push('<ol class="md-ol">')
        list = 'ol'
      }
      out.push(`<li>${inline(ol[2])}</li>`)
      i++
      continue
    }

    const q = QUOTE.exec(trimmed)
    if (q) {
      closeList()
      out.push(`<blockquote class="md-quote">${inline(q[1])}</blockquote>`)
      i++
      continue
    }

    // GFM table: a header row immediately followed by a |---|---| separator.
    if (trimmed.includes('|') && i + 1 < lines.length && isTableSeparator(lines[i + 1])) {
      closeList()
      const header = splitRow(line)
      const aligns = parseAligns(lines[i + 1])
      i += 2
      const rows: string[][] = []
      while (i < lines.length && lines[i].trim() !== '' && lines[i].includes('|')) {
        rows.push(splitRow(lines[i]))
        i++
      }
      out.push(renderTable(header, aligns, rows))
      continue
    }

    // Paragraph: gather consecutive lines that aren't blank or a block marker.
    closeList()
    const para: string[] = []
    while (i < lines.length) {
      const t = lines[i].trim()
      if (
        t === '' ||
        t.startsWith('$$') ||
        HEADING.test(t) ||
        UL_ITEM.test(t) ||
        OL_ITEM.test(t) ||
        QUOTE.test(t)
      )
        break
      para.push(lines[i])
      i++
    }
    out.push(`<p class="md-p">${para.map(inline).join('<br>')}</p>`)
  }

  closeList()
  return out.join('')
}
