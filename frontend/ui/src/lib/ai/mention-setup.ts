import { get } from 'svelte/store'

import * as cmd from '../tauri/commands'
import { workspace } from '../stores/workspace'
import { git } from '../stores/git'
import { lsp } from '../stores/lsp'
import { terminalSessions } from '../stores/terminal-sessions'
import { readActiveTerminalBuffer } from '../terminal/terminal-registry'
import {
  stripHtml,
  type MentionItem,
  type WorkspaceEntry,
} from '../stores/mention-providers'

/**
 * @-mention resolution (GWEN-332). The Tauri-touching half of the feature: it
 * walks the workspace for the fuzzy index, resolves each mention's content at
 * send time, and assembles the `<context>` block prepended to the message. The
 * pure registry/helpers live in `../stores/mention-providers.ts`.
 *
 * Resolution is best-effort: a mention that can't be read injects a short
 * `[unavailable: …]` note rather than throwing, so one bad mention never blocks
 * a send.
 */

// --- Limits ----------------------------------------------------------------

/** Max lines of a single file injected (truncated beyond this). */
const FILE_MAX_LINES = 500
/** Max lines per file when expanding a folder. */
const FOLDER_FILE_MAX_LINES = 200
/** Max files expanded for a folder mention (keeps the block bounded). */
const FOLDER_MAX_FILES = 40
/** Lines of terminal scrollback for @terminal. */
const TERMINAL_MAX_LINES = 50
/** Char cap on a fetched web page after stripping. */
const WEB_MAX_CHARS = 20000
/** Directories never walked for the index or folder expansion. */
const IGNORED_DIRS = new Set([
  'node_modules', '.git', 'target', 'dist', 'build', '.svelte-kit', '.next',
  '.gwenland', 'out', '.cache', 'coverage', '.venv', '__pycache__',
])

// --- Workspace file index (for fuzzy search) -------------------------------

let indexCache: { root: string; entries: WorkspaceEntry[] } | null = null
let indexInFlight: Promise<WorkspaceEntry[]> | null = null

/** Forward-slash relative path from the workspace root. */
function relPath(root: string, abs: string): string {
  let rel = abs
  if (abs.toLowerCase().startsWith(root.toLowerCase())) {
    rel = abs.slice(root.length).replace(/^[\\/]+/, '')
  }
  return rel.replace(/\\/g, '/')
}

/**
 * Recursively list the workspace (breadth-first via `listDirectory`), skipping
 * ignored/dot directories. Bounded so a huge tree can't hang the picker.
 */
async function buildIndex(root: string): Promise<WorkspaceEntry[]> {
  const entries: WorkspaceEntry[] = []
  const queue: string[] = [root]
  const MAX_ENTRIES = 5000
  while (queue.length && entries.length < MAX_ENTRIES) {
    const dir = queue.shift() as string
    let children: cmd.DirEntry[]
    try {
      children = await cmd.listDirectory(dir)
    } catch {
      continue
    }
    for (const c of children) {
      if (c.is_dir && (IGNORED_DIRS.has(c.name) || c.name.startsWith('.'))) continue
      entries.push({ path: c.path, rel: relPath(root, c.path), isDir: c.is_dir })
      if (c.is_dir) queue.push(c.path)
    }
  }
  return entries
}

/**
 * The workspace file index, cached per root. The first `@` builds it; later
 * mentions reuse the cache. Returns an empty list when no folder is open.
 */
export async function getWorkspaceIndex(): Promise<WorkspaceEntry[]> {
  const root = get(workspace).folderPath
  if (!root) return []
  if (indexCache && indexCache.root === root) return indexCache.entries
  if (indexInFlight) return indexInFlight
  indexInFlight = buildIndex(root).then((entries) => {
    indexCache = { root, entries }
    indexInFlight = null
    return entries
  })
  return indexInFlight
}

/** Invalidate the cached index (e.g. after files are created/deleted). */
export function invalidateWorkspaceIndex(): void {
  indexCache = null
}

// --- Per-type resolvers ----------------------------------------------------

function truncateLines(text: string, max: number): { text: string; truncated: boolean } {
  const lines = text.split('\n')
  if (lines.length <= max) return { text, truncated: false }
  return { text: lines.slice(0, max).join('\n'), truncated: true }
}

async function resolveFile(m: MentionItem): Promise<string> {
  if (!m.path) return '[unavailable: no path]'
  let content: string
  try {
    content = await cmd.readFile(m.path)
  } catch (e) {
    return `[unavailable: ${String(e)}]`
  }
  // Specific line range: slice it (1-based inclusive) and keep within the cap.
  if (m.lStart !== undefined && m.lEnd !== undefined) {
    const lines = content.split('\n')
    const slice = lines.slice(m.lStart - 1, m.lEnd)
    return truncateLines(slice.join('\n'), FILE_MAX_LINES).text
  }
  return truncateLines(content, FILE_MAX_LINES).text
}

async function resolveFolder(m: MentionItem): Promise<string> {
  if (!m.path) return '[unavailable: no path]'
  const root = get(workspace).folderPath ?? m.path
  let children: cmd.DirEntry[]
  try {
    children = await cmd.listDirectory(m.path)
  } catch (e) {
    return `[unavailable: ${String(e)}]`
  }
  // A small tree listing first, then the contents of each file (bounded).
  const tree = children
    .map((c) => `${c.is_dir ? '📁' : '📄'} ${c.name}`)
    .join('\n')
  const files = children.filter((c) => !c.is_dir).slice(0, FOLDER_MAX_FILES)
  const blocks: string[] = [`Tree:\n${tree}`]
  for (const f of files) {
    try {
      const raw = await cmd.readFile(f.path)
      const { text, truncated } = truncateLines(raw, FOLDER_FILE_MAX_LINES)
      blocks.push(
        `--- ${relPath(root, f.path)}${truncated ? ` (first ${FOLDER_FILE_MAX_LINES} lines)` : ''} ---\n${text}`
      )
    } catch {
      /* skip unreadable (e.g. binary) files */
    }
  }
  return blocks.join('\n\n')
}

async function resolveGit(): Promise<string> {
  const root = get(workspace).folderPath
  if (!root) return '[unavailable: no workspace]'
  const state = get(git)
  if (!state.isRepo) return '[unavailable: not a git repository]'
  if (state.files.length === 0) return '(working tree clean)'
  const diffs: string[] = []
  for (const f of state.files) {
    try {
      const d = await cmd.gitDiffFile(root, f.path, f.untracked)
      if (d.trim()) diffs.push(d.trimEnd())
    } catch {
      /* skip files whose diff can't be produced */
    }
  }
  return diffs.length ? diffs.join('\n') : '(no textual diff)'
}

function resolveDiagnostics(): string {
  const { diagnostics } = get(lsp)
  const lines: string[] = []
  for (const [path, diags] of Object.entries(diagnostics)) {
    for (const d of diags) {
      const where = `${path.split(/[\\/]/).pop()}:${d.range.start_line + 1}:${d.range.start_character + 1}`
      const code = d.code ? ` [${d.code}]` : ''
      lines.push(`${d.severity.toUpperCase()} ${where}${code} — ${d.message}`)
    }
  }
  return lines.length ? lines.join('\n') : '(no diagnostics)'
}

function resolveTerminal(): string {
  const activeKey = get(terminalSessions).activeKey ?? undefined
  const buf = readActiveTerminalBuffer(activeKey, TERMINAL_MAX_LINES)
  return buf || '(no terminal output)'
}

async function resolveWeb(m: MentionItem): Promise<string> {
  if (!m.url) return '[unavailable: no url]'
  try {
    const res = await fetch(m.url, { redirect: 'follow' })
    if (!res.ok) return `[unavailable: HTTP ${res.status}]`
    const html = await res.text()
    const text = stripHtml(html)
    return text.length > WEB_MAX_CHARS ? `${text.slice(0, WEB_MAX_CHARS)}\n…[truncated]` : text
  } catch (e) {
    return `[unavailable: ${String(e)}]`
  }
}

/** Resolve one mention's content (used both at send time and for previews). */
export async function resolveMention(m: MentionItem): Promise<string> {
  switch (m.type) {
    case 'file':
      return resolveFile(m)
    case 'folder':
      return resolveFolder(m)
    case 'git':
      return resolveGit()
    case 'diagnostics':
      return resolveDiagnostics()
    case 'terminal':
      return resolveTerminal()
    case 'web':
      return resolveWeb(m)
    default:
      return ''
  }
}

// --- Context block ---------------------------------------------------------

/** The `[@... ]` header line that labels each resolved mention in the block. */
function mentionHeader(m: MentionItem, content: string): string {
  switch (m.type) {
    case 'file': {
      const rel = headerPath(m.path)
      if (m.lStart !== undefined && m.lEnd !== undefined) {
        return `[@file: ${rel} L${m.lStart}-${m.lEnd}]`
      }
      const lineCount = content ? content.split('\n').length : 0
      return `[@file: ${rel} L1-${lineCount}]`
    }
    case 'folder':
      return `[@folder: ${headerPath(m.path)}]`
    case 'git':
      return '[@git]'
    case 'diagnostics':
      return '[@diagnostics]'
    case 'terminal':
      return '[@terminal]'
    case 'web':
      return `[@web: ${m.url}]`
    default:
      return '[@context]'
  }
}

function headerPath(abs?: string): string {
  if (!abs) return ''
  const root = get(workspace).folderPath
  return root ? relPath(root, abs) : abs
}

/**
 * Resolve every mention (in parallel) and return a single `<context>…</context>`
 * block, or '' when there are no mentions. Each mention's `resolvedContent` is
 * filled in as a side effect so the pills can reflect it if needed.
 */
export async function buildContextBlock(mentions: MentionItem[]): Promise<string> {
  if (mentions.length === 0) return ''
  const resolved = await Promise.all(
    mentions.map(async (m) => {
      const content = await resolveMention(m)
      m.resolvedContent = content
      return `${mentionHeader(m, content)}\n${content}`
    })
  )
  return `<context>\n${resolved.join('\n\n')}\n</context>`
}

/**
 * Prepend the resolved context block to the user's message. Returns the message
 * unchanged when there are no mentions.
 */
export async function resolveAllMentions(
  message: string,
  mentions: MentionItem[]
): Promise<string> {
  const block = await buildContextBlock(mentions)
  if (!block) return message
  return `${block}\n\n${message}`
}
