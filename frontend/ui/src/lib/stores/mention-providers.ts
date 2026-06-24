import type { IconName } from '../components/Icon.svelte'

/**
 * @-mention context providers (GWEN-332). This module is Tauri-free and
 * side-effect free — it owns the declarative provider registry plus the pure
 * helpers the composer uses while typing (`parseMentionQuery`, `fuzzySearch`,
 * `parseLineRange`, `stripHtml`, icon mapping). All resolution side effects
 * (reading files, fetching URLs, building the context block) live in
 * `../ai/mention-setup.ts`, mirroring the slash-commands store/setup split.
 */

/** The kind of context a resolved mention injects. */
export type MentionType = 'file' | 'folder' | 'git' | 'diagnostics' | 'terminal' | 'web'

/**
 * A resolved mention attached to the next message. `path` is the absolute path
 * for file/folder mentions; `url` the target for web; line bounds are 1-based
 * and only set for a `@file:start-end` range. `resolvedContent` is filled in at
 * send time by the setup layer (kept null in the store until then).
 */
export interface MentionItem {
  id: string
  type: MentionType
  /** Absolute path (file/folder) — undefined for git/diagnostics/terminal/web. */
  path?: string
  /** Fetched URL for `@web`. */
  url?: string
  /** 1-based inclusive start line for a `@file:start-end` range. */
  lStart?: number
  /** 1-based inclusive end line for a `@file:start-end` range. */
  lEnd?: number
  /** Short label shown on the pill (filename only, plus `:start-end`). */
  label: string
  /** Resolved text injected into the <context> block (set at send time). */
  resolvedContent?: string
}

/**
 * A candidate offered in the dropdown. Special providers are fixed; file/folder
 * candidates are produced from the workspace index by `fuzzySearch`.
 */
export interface MentionCandidate {
  /** Discriminates rendering + how the composer turns it into a MentionItem. */
  type: MentionType
  /** The text inserted after `@` when chosen (e.g. `git`, `src/main.rs`). */
  insert: string
  /** Row label (the special name, or the path relative to the workspace root). */
  label: string
  /** One-line description for special providers; the dir for files. */
  detail: string
  icon: IconName
  /** Absolute path for file/folder candidates. */
  path?: string
}

/** The four non-file providers, listed before fuzzy file results. */
export const SPECIAL_PROVIDERS: MentionCandidate[] = [
  { type: 'git', insert: 'git', label: '@git', detail: 'Working tree diff', icon: 'git-branch' },
  {
    type: 'diagnostics',
    insert: 'diagnostics',
    label: '@diagnostics',
    detail: 'LSP errors & warnings',
    icon: 'warning-triangle',
  },
  {
    type: 'terminal',
    insert: 'terminal',
    label: '@terminal',
    detail: 'Last 50 terminal lines',
    icon: 'terminal',
  },
  { type: 'web', insert: 'web ', label: '@web <url>', detail: 'Fetch a URL', icon: 'globe' },
]

// --- File-type icons -------------------------------------------------------
// The icon set (Iconoir) has no per-language glyphs, so code files share the
// `code` icon and everything else uses `page`; folders use `folder`. This keeps
// to the project's existing icon registry (no new icon dependency).
const CODE_EXTS = new Set([
  'ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs', 'rs', 'py', 'go', 'java', 'c', 'h',
  'cpp', 'hpp', 'cs', 'rb', 'php', 'swift', 'kt', 'svelte', 'vue', 'sh',
])

/** Icon for a path by extension (folders → folder, code → code, else page). */
export function iconForPath(path: string, isDir: boolean): IconName {
  if (isDir) return 'folder'
  const base = path.split(/[\\/]/).pop() ?? path
  const dot = base.lastIndexOf('.')
  const ext = dot > 0 ? base.slice(dot + 1).toLowerCase() : ''
  return CODE_EXTS.has(ext) ? 'code' : 'page'
}

/** Icon for a resolved mention (drives the pill glyph). */
export function iconForMention(m: MentionItem): IconName {
  switch (m.type) {
    case 'folder':
      return 'folder'
    case 'git':
      return 'git-branch'
    case 'diagnostics':
      return 'warning-triangle'
    case 'terminal':
      return 'terminal'
    case 'web':
      return 'globe'
    case 'file':
    default:
      return iconForPath(m.path ?? m.label, false)
  }
}

// --- Mention query parsing -------------------------------------------------

/**
 * The active `@` query under the caret, or null when there isn't one. We scan
 * back from the caret to the nearest unescaped `@` that begins a token (start
 * of text or after whitespace) and read up to the caret. A space ends a normal
 * mention — except `@web <url>`, which intentionally allows one trailing arg.
 */
export interface MentionQuery {
  /** Caret index of the `@` (used to splice the chosen insert back in). */
  at: number
  /** Text typed after `@`, up to the caret (may contain a space for @web). */
  query: string
}

export function parseMentionQuery(text: string, caret: number): MentionQuery | null {
  // Walk left to find a candidate `@`.
  let i = caret - 1
  while (i >= 0) {
    const ch = text[i]
    if (ch === '@') break
    // A newline always ends the search; whitespace only ends it for non-@web.
    if (ch === '\n') return null
    i--
  }
  if (i < 0 || text[i] !== '@') return null
  // The `@` must start a token (beginning of text or preceded by whitespace).
  if (i > 0 && !/\s/.test(text[i - 1])) return null

  const raw = text.slice(i + 1, caret)
  // For a normal mention, a space terminates it — unless it's the `@web ` form,
  // where the user types a URL after the space.
  if (/\s/.test(raw) && !/^web\s/i.test(raw)) return null
  return { at: i, query: raw }
}

// --- Line range ------------------------------------------------------------

/**
 * Parse a `path:start-end` (or `path:line`) suffix into a base path + line
 * bounds. Returns the base path unchanged with undefined bounds when there's no
 * valid `:range`. A bare `:N` selects a single line (start === end).
 *
 *   "src/main.rs"        → { path: "src/main.rs" }
 *   "src/main.rs:10-50"  → { path: "src/main.rs", lStart: 10, lEnd: 50 }
 *   "src/main.rs:42"     → { path: "src/main.rs", lStart: 42, lEnd: 42 }
 *
 * Windows drive letters (`C:\...`) are not mistaken for a range — the colon is
 * only treated as a range marker when what follows is `\d+(-\d+)?`.
 */
export interface ParsedRange {
  path: string
  lStart?: number
  lEnd?: number
}

export function parseLineRange(raw: string): ParsedRange {
  const m = raw.match(/^(.*?):(\d+)(?:-(\d+))?$/)
  if (!m) return { path: raw }
  const base = m[1]
  if (!base) return { path: raw }
  const start = parseInt(m[2], 10)
  const end = m[3] !== undefined ? parseInt(m[3], 10) : start
  // Normalize so start <= end.
  const lStart = Math.min(start, end)
  const lEnd = Math.max(start, end)
  return { path: base, lStart, lEnd }
}

// --- Fuzzy search ----------------------------------------------------------

/** A workspace path entry the fuzzy search ranks. */
export interface WorkspaceEntry {
  /** Absolute path. */
  path: string
  /** Path relative to the workspace root (forward slashes), used for display + match. */
  rel: string
  isDir: boolean
}

/**
 * Score `query` against a candidate string. Higher is better; 0 means no match.
 * Ranking, strongest first: exact, prefix, word-boundary/segment start,
 * substring, subsequence (chars in order). Shorter targets and earlier matches
 * win ties so the closest file floats up.
 */
export function fuzzyScore(query: string, target: string): number {
  if (!query) return 1
  const q = query.toLowerCase()
  const t = target.toLowerCase()
  if (t === q) return 1000
  if (t.startsWith(q)) return 800 - t.length
  // Match at a path/word boundary (after / \ . _ - or camelCase hump).
  const boundary = new RegExp(`(^|[/\\\\._-])${escapeRe(q)}`).test(t)
  if (boundary) return 600 - t.length
  const idx = t.indexOf(q)
  if (idx >= 0) return 400 - idx - t.length * 0.1
  // Subsequence: all query chars appear in order.
  if (isSubsequence(q, t)) return 200 - t.length * 0.1
  return 0
}

function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function isSubsequence(q: string, t: string): boolean {
  let i = 0
  for (let j = 0; j < t.length && i < q.length; j++) {
    if (t[j] === q[i]) i++
  }
  return i === q.length
}

/**
 * Rank workspace entries against a query, matching on the basename first then
 * the full relative path (basename matches score higher). Returns the top
 * `limit` candidates, files and folders interleaved by score.
 */
export function fuzzySearch(
  query: string,
  entries: WorkspaceEntry[],
  limit = 20
): MentionCandidate[] {
  const scored: { e: WorkspaceEntry; score: number }[] = []
  for (const e of entries) {
    const base = e.rel.split('/').pop() ?? e.rel
    const score = Math.max(fuzzyScore(query, base), fuzzyScore(query, e.rel) * 0.9)
    if (score > 0) scored.push({ e, score })
  }
  scored.sort((a, b) => b.score - a.score || a.e.rel.length - b.e.rel.length)
  return scored.slice(0, limit).map(({ e }) => ({
    type: e.isDir ? ('folder' as const) : ('file' as const),
    insert: e.isDir ? `${e.rel}/` : e.rel,
    label: e.rel.split('/').pop() ?? e.rel,
    detail: e.rel,
    icon: iconForPath(e.path, e.isDir),
    path: e.path,
  }))
}

// --- HTML stripping (for @web) ---------------------------------------------

/** Common named HTML entities → their characters (the high-frequency set). */
const ENTITIES: Record<string, string> = {
  amp: '&',
  lt: '<',
  gt: '>',
  quot: '"',
  apos: "'",
  nbsp: ' ',
  copy: '©',
  reg: '®',
  trade: '™',
  hellip: '…',
  mdash: '—',
  ndash: '–',
  lsquo: '‘',
  rsquo: '’',
  ldquo: '“',
  rdquo: '”',
}

/** Decode named + numeric (decimal/hex) HTML entities. */
function decodeEntities(s: string): string {
  return s.replace(/&(#x?[0-9a-fA-F]+|[a-zA-Z]+);/g, (whole, body: string) => {
    if (body[0] === '#') {
      const code =
        body[1] === 'x' || body[1] === 'X'
          ? parseInt(body.slice(2), 16)
          : parseInt(body.slice(1), 10)
      return Number.isFinite(code) ? safeFromCodePoint(code) : whole
    }
    const named = ENTITIES[body.toLowerCase()]
    return named ?? whole
  })
}

function safeFromCodePoint(code: number): string {
  try {
    return String.fromCodePoint(code)
  } catch {
    return ''
  }
}

/**
 * Strip HTML to readable plain text, from scratch (no DOMParser, no deps):
 *  1. drop <script>/<style>/<head>/<noscript> blocks (content + tags),
 *  2. turn block-level closes + <br> into newlines so structure survives,
 *  3. remove all remaining tags and comments,
 *  4. decode entities, then collapse runs of whitespace (max one blank line).
 */
export function stripHtml(html: string): string {
  let s = html
  // 1. Remove element blocks whose text content is not body copy.
  s = s.replace(/<(script|style|head|noscript)\b[^>]*>[\s\S]*?<\/\1>/gi, ' ')
  // HTML comments.
  s = s.replace(/<!--[\s\S]*?-->/g, ' ')
  // 2. Preserve line structure for common block elements + explicit breaks.
  s = s.replace(/<\/(p|div|section|article|header|footer|li|tr|h[1-6])\s*>/gi, '\n')
  s = s.replace(/<br\s*\/?>/gi, '\n')
  s = s.replace(/<\/(ul|ol|table|thead|tbody)\s*>/gi, '\n')
  // 3. Strip every remaining tag.
  s = s.replace(/<[^>]+>/g, '')
  // 4. Decode entities, then normalize whitespace.
  s = decodeEntities(s)
  // Collapse spaces/tabs, trim each line, and cap consecutive blank lines at 1.
  s = s
    .split('\n')
    .map((line) => line.replace(/[ \t\f\v]+/g, ' ').trim())
    .join('\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
  return s
}
