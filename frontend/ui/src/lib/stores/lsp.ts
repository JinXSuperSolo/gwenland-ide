import { writable, get } from 'svelte/store'
import {
  lspOpenDocument,
  lspChangeDocument,
  lspCloseDocument,
  onLspDiagnostics,
  onLspStatus,
  type LspDiagnostic,
  type LspLanguage,
  type LspStatus,
} from '../tauri/commands'
import { workspace } from './workspace'

/**
 * UI-side LSP state (Milestone 6, Wave 4). Diagnostics and status are keyed by
 * absolute file path so the editor and the status indicator can look up the
 * active file. The engine owns all protocol state; this store only caches what
 * the UI renders and tracks per-document versions for change notifications.
 */
interface LspStoreState {
  /** Per-file status (from open results and status events). */
  status: Record<string, LspStatus>
  /** Per-file latest diagnostics (empty array = cleared). */
  diagnostics: Record<string, LspDiagnostic[]>
}

export const lsp = writable<LspStoreState>({ status: {}, diagnostics: {} })

/** Normalize a file path to forward-slash form for use as a store key.
 *  The Rust side emits paths from `to_string_lossy()` on a PathBuf that was
 *  built from a forward-slash URI string, so it always arrives with `/`.
 *  The frontend file tree uses OS-native separators (backslashes on Windows).
 *  Keying both sides through this helper makes lookups separator-agnostic. */
export function normPath(p: string): string {
  return p.replace(/\\/g, '/')
}

/** Monotonic document versions per path (Requirement 9.4). UI-generated so Rust
 *  never invents conflicting versions (Requirement 13.5). */
const versions = new Map<string, number>()

function nextVersion(path: string): number {
  const v = (versions.get(path) ?? 0) + 1
  versions.set(path, v)
  return v
}

/** Map a file path to its LSP language id (mirror of the engine's extension
 *  map). Returns null for unsupported files (plain-editor mode). */
export function languageForPath(path: string): LspLanguage | null {
  const base = path.split(/[\\/]/).pop() ?? path
  const dot = base.lastIndexOf('.')
  const ext = dot > 0 ? base.slice(dot + 1).toLowerCase() : ''
  switch (ext) {
    case 'rs':
      return 'rust'
    case 'ts':
    case 'tsx':
      return 'typescript'
    case 'js':
    case 'jsx':
    case 'mjs':
    case 'cjs':
      return 'javascript'
    case 'py':
      return 'python'
    default:
      return null
  }
}

/** The server bucket key for restart (TS and JS share the `typescript` server). */
export function serverKeyForLanguage(language: LspLanguage): string {
  return language === 'javascript' ? 'typescript' : language
}

function setStatus(path: string, status: LspStatus): void {
  lsp.update((s) => ({ ...s, status: { ...s.status, [normPath(path)]: status } }))
}

/**
 * Open a document for LSP (called when a file tab is created). Non-blocking:
 * unsupported files get a local status and never invoke the backend. Failures
 * are swallowed so editing is never disrupted (Requirement 12.6).
 */
export async function lspOpenPath(path: string, text: string): Promise<void> {
  if (!path) return
  const language = languageForPath(path)
  if (!language) {
    setStatus(path, { state: 'unsupported_language' })
    return
  }
  const npath = normPath(path)
  versions.delete(npath)
  const version = nextVersion(npath) // 1
  // Optimistic "starting" so the indicator reacts immediately on slow servers.
  setStatus(path, { state: 'starting', language })
  try {
    const status = await lspOpenDocument(path, text, version, get(workspace).folderPath)
    setStatus(path, status)
  } catch {
    // Backend rejection (e.g. settings load): leave editing fully usable.
  }
}

/** Push a full-text change for an open LSP document (debounced by the caller). */
export async function lspChangePath(path: string, text: string): Promise<void> {
  if (!path || !languageForPath(path)) return
  const version = nextVersion(normPath(path))
  try {
    await lspChangeDocument(path, text, version)
  } catch {
    // Transport failure surfaces as a status event; typing must not fail.
  }
}

/** Close an LSP document (called when a tab closes): clears its UI state. */
export async function lspClosePath(path: string): Promise<void> {
  if (!path) return
  const npath = normPath(path)
  versions.delete(npath)
  lsp.update((s) => {
    const status = { ...s.status }
    const diagnostics = { ...s.diagnostics }
    delete status[npath]
    delete diagnostics[npath]
    return { status, diagnostics }
  })
  if (!languageForPath(path)) return
  try {
    await lspCloseDocument(path)
  } catch {
    /* best-effort */
  }
}

let initialized = false

/**
 * Register the global `lsp://diagnostics` and `lsp://status` listeners. Called
 * once at startup. Diagnostics are routed by path; status transitions
 * (crash/restart/connect) update every open file of the matching language.
 */
export function initLsp(): void {
  if (initialized) return
  initialized = true

  onLspDiagnostics((event) => {
    lsp.update((s) => ({
      ...s,
      diagnostics: { ...s.diagnostics, [normPath(event.path)]: event.diagnostics },
    }))
  }).catch(() => {})

  onLspStatus((event) => {
    if (!event.language) return
    // The event payload is itself a full LspStatus (status fields flattened).
    const next: LspStatus = event as LspStatus
    lsp.update((s) => {
      const status = { ...s.status }
      for (const [path, cur] of Object.entries(status)) {
        if ('language' in cur && cur.language === event.language) {
          status[path] = next
        }
      }
      return { ...s, status }
    })
  }).catch(() => {})
}
