/**
 * Fenced-code-block parsing + lightweight syntax highlighting for AI messages
 * (Milestone 4, Wave 4).
 *
 * `parseSegments` splits an assistant message into alternating prose/code
 * segments, tolerating an *incomplete* trailing fence so partial code renders
 * safely while streaming (Requirement 13.6).
 *
 * `highlight` is a small, dependency-free tokenizer. The committed CodeMirror
 * bundle ships the framework but no per-language grammars, so rather than pull a
 * heavy highlighter we colour comments / strings / numbers / keywords across the
 * required languages. Unknown languages fall back to plain (escaped) monospace
 * (Requirement 13.8). Output is HTML built from HTML-escaped source — safe to
 * render with `{@html}`.
 */

export interface Segment {
  kind: 'text' | 'code'
  /** Language hint from the opening fence (code segments only). */
  lang: string
  content: string
}

const FENCE = '```'

/** Split message content into prose/code segments by ``` fences. */
export function parseSegments(input: string): Segment[] {
  const segments: Segment[] = []
  const lines = input.split('\n')
  let inCode = false
  let lang = ''
  let buf: string[] = []

  const flushText = () => {
    const content = buf.join('\n')
    if (content.trim().length > 0) segments.push({ kind: 'text', lang: '', content })
    buf = []
  }
  const flushCode = () => {
    segments.push({ kind: 'code', lang, content: buf.join('\n') })
    buf = []
  }

  for (const line of lines) {
    if (line.trimStart().startsWith(FENCE)) {
      if (!inCode) {
        flushText()
        inCode = true
        lang = line.trimStart().slice(FENCE.length).trim().split(/\s+/)[0] ?? ''
      } else {
        flushCode()
        inCode = false
        lang = ''
      }
      continue
    }
    buf.push(line)
  }

  // Trailing buffer: a still-open fence (streaming) renders as code.
  if (inCode) flushCode()
  else flushText()

  return segments
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

interface LangSpec {
  lineComment: string[]
  block: [string, string] | null
  strings: string[]
  keywords: Set<string>
}

const C_LIKE_KEYWORDS = [
  'const', 'let', 'var', 'function', 'fn', 'return', 'if', 'else', 'for', 'while',
  'loop', 'match', 'switch', 'case', 'break', 'continue', 'struct', 'enum', 'impl',
  'trait', 'class', 'interface', 'type', 'pub', 'use', 'import', 'export', 'from',
  'mut', 'async', 'await', 'new', 'this', 'self', 'super', 'static', 'where',
  'extends', 'implements', 'public', 'private', 'protected', 'try', 'catch',
  'finally', 'throw', 'in', 'of', 'as', 'default', 'void', 'null', 'undefined',
  'true', 'false', 'dyn', 'move', 'ref', 'Some', 'None', 'Ok', 'Err',
]
const PY_KEYWORDS = [
  'def', 'class', 'return', 'if', 'elif', 'else', 'for', 'while', 'break',
  'continue', 'import', 'from', 'as', 'with', 'try', 'except', 'finally',
  'raise', 'lambda', 'yield', 'pass', 'global', 'nonlocal', 'in', 'is', 'and',
  'or', 'not', 'None', 'True', 'False', 'self', 'async', 'await',
]

function langSpec(lang: string): LangSpec | null {
  switch (lang.toLowerCase()) {
    case 'rust':
    case 'rs':
    case 'ts':
    case 'typescript':
    case 'js':
    case 'javascript':
    case 'jsx':
    case 'tsx':
      return {
        lineComment: ['//'],
        block: ['/*', '*/'],
        strings: ['"', "'", '`'],
        keywords: new Set(C_LIKE_KEYWORDS),
      }
    case 'python':
    case 'py':
      return { lineComment: ['#'], block: null, strings: ['"', "'"], keywords: new Set(PY_KEYWORDS) }
    case 'css':
      return { lineComment: [], block: ['/*', '*/'], strings: ['"', "'"], keywords: new Set() }
    case 'json':
      return { lineComment: [], block: null, strings: ['"'], keywords: new Set(['true', 'false', 'null']) }
    case 'html':
    case 'xml':
      return { lineComment: [], block: ['<!--', '-->'], strings: ['"', "'"], keywords: new Set() }
    default:
      return null
  }
}

function span(cls: string, raw: string): string {
  return `<span class="${cls}">${escapeHtml(raw)}</span>`
}

/** Produce highlighted HTML for `code`. Unknown langs return escaped plaintext. */
export function highlight(code: string, lang: string): string {
  const spec = langSpec(lang)
  if (!spec) return escapeHtml(code)

  let out = ''
  let i = 0
  const n = code.length
  const isIdentStart = (c: string) => /[A-Za-z_$]/.test(c)
  const isIdent = (c: string) => /[A-Za-z0-9_$]/.test(c)

  while (i < n) {
    const rest = code.slice(i)

    // Block comment
    if (spec.block && rest.startsWith(spec.block[0])) {
      const end = code.indexOf(spec.block[1], i + spec.block[0].length)
      const stop = end === -1 ? n : end + spec.block[1].length
      out += span('tok-comment', code.slice(i, stop))
      i = stop
      continue
    }
    // Line comment
    const lc = spec.lineComment.find((p) => rest.startsWith(p))
    if (lc) {
      let end = code.indexOf('\n', i)
      if (end === -1) end = n
      out += span('tok-comment', code.slice(i, end))
      i = end
      continue
    }
    // String
    const quote = spec.strings.find((q) => rest.startsWith(q))
    if (quote) {
      let j = i + quote.length
      while (j < n) {
        if (code[j] === '\\') {
          j += 2
          continue
        }
        if (code.startsWith(quote, j)) {
          j += quote.length
          break
        }
        j++
      }
      out += span('tok-string', code.slice(i, Math.min(j, n)))
      i = Math.min(j, n)
      continue
    }
    // Number
    if (/[0-9]/.test(code[i])) {
      let j = i
      while (j < n && /[0-9a-fA-FxX._]/.test(code[j])) j++
      out += span('tok-number', code.slice(i, j))
      i = j
      continue
    }
    // Identifier / keyword
    if (isIdentStart(code[i])) {
      let j = i
      while (j < n && isIdent(code[j])) j++
      const word = code.slice(i, j)
      out += spec.keywords.has(word) ? span('tok-keyword', word) : escapeHtml(word)
      i = j
      continue
    }
    // Any other single char
    out += escapeHtml(code[i])
    i++
  }

  return out
}
