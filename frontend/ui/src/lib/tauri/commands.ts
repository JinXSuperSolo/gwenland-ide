import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { CommitDetails, CommitGraphPayload } from '../types/git'

/**
 * Thin typed wrappers over the existing Rust `#[tauri::command]`s in
 * `frontend/src/main.rs`. These give the Svelte side a typed surface over the
 * backend commands. Argument names match the Rust fn params
 * (Tauri maps camelCase JS keys, but these are already single-word snake-safe).
 */

/** Mirrors `gwenland_engine::fs::DirEntry` (see engine/src/fs.rs). */
export interface DirEntry {
  name: string
  path: string
  is_dir: boolean
}

/**
 * Opens the native folder-picker. Resolves to the chosen absolute path, or
 * rejects with the engine's "folder selection was cancelled" string if the
 * user dismisses the dialog.
 */
export function openFolderDialog(): Promise<string> {
  return invoke<string>('open_folder_dialog')
}

/**
 * Lists the immediate children of `path`. The Rust side already returns
 * directories first, then files, each group ascending case-insensitively —
 * do NOT re-sort on the frontend.
 */
export function listDirectory(path: string): Promise<DirEntry[]> {
  return invoke<DirEntry[]>('list_directory', { path })
}

/**
 * One coalesced batch of changes for a single directory (M19 Wave 1).
 * Mirrors `gwenland_engine::fs_watch::FsPatch`. All paths are absolute; `dir`
 * is the affected parent directory.
 */
export interface FsPatch {
  dir: string
  added: string[]
  removed: string[]
  modified: string[]
  modified_dirs?: string[]
}

/** Begin watching `path` for changes (registered on folder expand). */
export function fsWatchDir(path: string): Promise<void> {
  return invoke<void>('fs_watch_dir', { path })
}

/** Stop watching `path` (called on folder collapse). */
export function fsUnwatchDir(path: string): Promise<void> {
  return invoke<void>('fs_unwatch_dir', { path })
}

/** Stop watching every directory (workspace closed/switched). */
export function fsWatchClear(): Promise<void> {
  return invoke<void>('fs_watch_clear')
}

/**
 * Subscribe to coalesced file-system patches. The watcher emits one `fs:patch`
 * event per poll cycle carrying every changed directory's batch — never one
 * event per file. Returns the Tauri unlisten handle.
 */
export function onFsPatch(handler: (patches: FsPatch[]) => void): Promise<UnlistenFn> {
  return listen<FsPatch[]>('fs:patch', (e) => handler(e.payload))
}

/**
 * One row in the Rust-owned flat tree (M19 Wave 2).
 * Mirrors `gwenland_engine::tree::FlatRow`. Git status and icons are NOT here —
 * they're derived JS-side (git store + filename), so the row is structural only.
 */
export interface FlatRow {
  id: string
  name: string
  path: string
  depth: number
  is_dir: boolean
  is_expanded: boolean
  has_children: boolean
  is_stale?: boolean
  is_loading?: boolean
  error?: string | null
}

/**
 * A delta to splice into the flat-row mirror. Mirrors
 * `gwenland_engine::tree::TreePatch` (serde tag = "kind", snake_case). Patches in
 * one array apply in order.
 */
export type TreePatch =
  | { kind: 'insert'; index: number; rows: FlatRow[] }
  | { kind: 'remove'; index: number; count: number }
  | { kind: 'update'; index: number; row: FlatRow }

/** Open a workspace root; returns its immediate child rows (initial render). */
export function treeSetRoot(path: string): Promise<FlatRow[]> {
  return invoke<FlatRow[]>('tree_set_root', { path })
}

/** Expand a folder; returns the patches that insert its children. */
export function treeExpand(path: string): Promise<TreePatch[]> {
  return invoke<TreePatch[]>('tree_expand', { path })
}

/** Collapse a folder; returns the patches that remove its subtree. */
export function treeCollapse(path: string): Promise<TreePatch[]> {
  return invoke<TreePatch[]>('tree_collapse', { path })
}

/** Reconcile a directory against disk; returns minimal add/remove patches. */
export function treeRefreshDir(path: string): Promise<TreePatch[]> {
  return invoke<TreePatch[]>('tree_refresh_dir', { path })
}

/** Records a folder in the recent-projects list. */
export function addRecentProject(path: string): Promise<void> {
  return invoke<void>('add_recent_project', { path })
}

/** Mirrors `gwenland_engine::recent_projects::RecentProject`. */
export interface RecentProject {
  path: string
  /** ISO-ish timestamp string from the engine. */
  last_opened: string
}

/** Returns the most-recent-first list of previously opened folders. */
export function getRecentProjects(): Promise<RecentProject[]> {
  return invoke<RecentProject[]>('get_recent_projects')
}

/**
 * Reads `path` as UTF-8 text. Rejects with the engine's "binary file…" message
 * if the bytes aren't valid UTF-8 (see engine/src/fs.rs FsError::BinaryFile).
 */
export function readFile(path: string): Promise<string> {
  return invoke<string>('read_file', { path })
}

/**
 * A file's size + line count (M19 Wave 3). Mirrors `gwenland_engine::fs::FileMeta`.
 * `line_count` is `Number.MAX_SAFE_INTEGER`-ish (Rust `u64::MAX`) when counting
 * was skipped for a very large file.
 */
export interface FileMeta {
  size: number
  line_count: number
}

/** Return a file's size + line count, used to pick Large File Mode on open. */
export function getFileMeta(path: string): Promise<FileMeta> {
  return invoke<FileMeta>('get_file_meta', { path })
}

/** Writes `content` to `path` atomically (tmp-write + rename, engine-side). */
export function writeFile(path: string, content: string): Promise<void> {
  return invoke<void>('write_file', { path, content })
}

/** Whether a path exists on disk. Used for fail-open restore checks. */
export function pathExists(path: string): Promise<boolean> {
  return invoke<boolean>('path_exists', { path })
}

// ---------------------------------------------------------------------------
// Workspace-scoped file operations (Milestone 9 — Context Menu System)
//
// Each mirrors a `#[tauri::command]` in main.rs that delegates to the engine's
// workspace-scoped fs ops. Every one takes `workspaceRoot`; the engine rejects
// (rejecting the promise) any target that resolves outside it. The context-menu
// actions are the only callers.
// ---------------------------------------------------------------------------

/** Create an empty file (New File). Rejects if it exists or is outside the root. */
export function createFile(path: string, workspaceRoot: string): Promise<void> {
  return invoke<void>('create_file', { path, workspaceRoot })
}

/** Create a directory (New Folder). Rejects if it exists or is outside the root. */
export function createDir(path: string, workspaceRoot: string): Promise<void> {
  return invoke<void>('create_dir', { path, workspaceRoot })
}

/** Rename/move `oldPath` to `newPath` (both must be inside the workspace). */
export function renamePath(
  oldPath: string,
  newPath: string,
  workspaceRoot: string
): Promise<void> {
  return invoke<void>('rename_path', { old: oldPath, new: newPath, workspaceRoot })
}

/** Delete a file or directory (recursive). Rejects outside the workspace. */
export function deletePath(path: string, workspaceRoot: string): Promise<void> {
  return invoke<void>('delete_path', { path, workspaceRoot })
}

/** Duplicate a path next to itself; resolves to the new path. */
export function duplicatePath(path: string, workspaceRoot: string): Promise<string> {
  return invoke<string>('duplicate_path', { path, workspaceRoot })
}

/** Reveal a path in the OS file manager (Explorer/Finder/xdg). */
export function revealInExplorer(path: string, workspaceRoot: string): Promise<void> {
  return invoke<void>('reveal_in_explorer', { path, workspaceRoot })
}

/** Move a path to the OS-native Recycle Bin / Trash. Manual file-tree action. */
export function moveToTrash(path: string, workspaceRoot: string): Promise<void> {
  return invoke<void>('move_path_to_os_trash', { path, workspaceRoot })
}

export interface TrashRecord {
  id: string
  timestamp: string
  original_path: string
  trash_path: string
  actor: string
}

/** Move a path to the workspace-local `.gwenland/trash/` recovery area. */
export function moveToWorkspaceTrash(path: string, workspaceRoot: string): Promise<TrashRecord> {
  return invoke<TrashRecord>('move_to_trash', { path, workspaceRoot })
}

export interface WorkspaceSearchResult {
  path: string
  relative_path: string
  line_number: number
  line: string
}

export interface WorkspaceSearchSummary {
  result_count: number
  scanned_files: number
  cancelled: boolean
  truncated: boolean
}

export interface WorkspaceSearchResultEvent {
  search_id: string
  result: WorkspaceSearchResult
}

export interface WorkspaceSearchDoneEvent {
  search_id: string
  summary: WorkspaceSearchSummary | null
  error: string | null
}

export function searchWorkspace(root: string, query: string, searchId: string): Promise<string> {
  return invoke<string>('search_workspace', { root, query, searchId })
}

export function searchCancel(searchId?: string | null): Promise<void> {
  return invoke<void>('search_cancel', { searchId: searchId ?? null })
}

/** Add a workspace-relative path to `.gwenland/safety/protected-paths.json`. */
export function markProtectedPath(path: string, workspaceRoot: string): Promise<void> {
  return invoke<void>('mark_protected_path', { path, workspaceRoot })
}

// ---------------------------------------------------------------------------
// Terminal I/O bridge (Milestone 3, Wave 2)
//
// Wraps the `terminal_*` Rust commands and the `terminal://output` event.
// PtySession output is pushed from Rust as raw bytes (a Vec<u8>, which arrives
// as a number[]), so the renderer (XTerm.js in Wave 3) decodes it. Sessions are
// keyed by the string id `terminalCreate` returns.
// ---------------------------------------------------------------------------

/** The Tauri event name PTY output is streamed on. */
export const TERMINAL_OUTPUT_EVENT = 'terminal://output'

/** Raw `terminal://output` payload as emitted by Rust (`Vec<u8>` → number[]). */
interface TerminalOutputPayload {
  id: string
  data: number[]
}

/**
 * Spawns the platform default shell in a new PTY of the given size and starts
 * streaming its output. `cwd`, when given, is the shell's working directory
 * (e.g. the open project folder); a missing/non-existent path falls back to the
 * default directory backend-side. Resolves to the new session id.
 */
export function terminalCreate(
  rows: number,
  cols: number,
  cwd?: string | null,
  shell?: string | null
): Promise<string> {
  return invoke<string>('terminal_create', { rows, cols, cwd: cwd ?? null, shell: shell ?? null })
}

export interface TerminalShellInfo {
  id: string
  label: string
  command: string
}

export function terminalDetectShells(): Promise<TerminalShellInfo[]> {
  return invoke<TerminalShellInfo[]>('terminal_detect_shells')
}

/** Sends raw input bytes (keystrokes / pasted text) to a session's PTY. */
export function terminalWrite(id: string, data: Uint8Array): Promise<void> {
  // Tauri serialises a JS number[] into the Rust `Vec<u8>` parameter.
  return invoke<void>('terminal_write', { id, data: Array.from(data) })
}

/** Resizes a session's PTY when the terminal panel is resized. */
export function terminalResize(id: string, rows: number, cols: number): Promise<void> {
  return invoke<void>('terminal_resize', { id, rows, cols })
}

/** Kills and disposes a session. Killing an unknown id is a no-op server-side. */
export function terminalKill(id: string): Promise<void> {
  return invoke<void>('terminal_kill', { id })
}

/**
 * Subscribes to PTY output for one session. `handler` receives each chunk as a
 * `Uint8Array` as it arrives. Returns the unlisten function — call it on
 * teardown to stop receiving events. Output for other sessions is filtered out.
 */
export async function onTerminalOutput(
  id: string,
  handler: (bytes: Uint8Array) => void
): Promise<UnlistenFn> {
  return listen<TerminalOutputPayload>(TERMINAL_OUTPUT_EVENT, (event) => {
    if (event.payload.id === id) {
      handler(Uint8Array.from(event.payload.data))
    }
  })
}

/** The Tauri event name detected errors are reported on (Wave 6, GWEN-251). */
export const TERMINAL_ERROR_EVENT = 'terminal://error'

/** A detected error in a session's output (mirrors Rust `TerminalErrorEvent`). */
export interface TerminalError {
  id: string
  /** Matched signature class, e.g. "rust-panic", "command-not-found". */
  label: string
  /** The offending (ANSI-stripped) line. */
  line: string
}

/**
 * Subscribes to detected-error events for one session. The backend flags likely
 * error lines reactively (a future AI "explain this error" feature will consume
 * this; no UI does yet). Returns the unlisten function. Errors for other
 * sessions are filtered out.
 */
export async function onTerminalError(
  id: string,
  handler: (error: TerminalError) => void
): Promise<UnlistenFn> {
  return listen<TerminalError>(TERMINAL_ERROR_EVENT, (event) => {
    if (event.payload.id === id) {
      handler(event.payload)
    }
  })
}

/**
 * Subscribe to detected errors across ALL terminal sessions. Used by the AI
 * panel's terminal-error bridge (M4), which is not session-scoped. Returns the
 * unlisten function.
 */
export async function onAnyTerminalError(
  handler: (error: TerminalError) => void
): Promise<UnlistenFn> {
  return listen<TerminalError>(TERMINAL_ERROR_EVENT, (event) => handler(event.payload))
}

// ---------------------------------------------------------------------------
// Dev-server ready detection (Milestone 5 — Web Preview)
//
// Wraps the `terminal://devserver-ready` event the backend emits (once per
// session) when it detects a dev server's ready URL in terminal output. The
// Web Preview feature listens for this to auto-open a preview pane.
// ---------------------------------------------------------------------------

/** The Tauri event name dev-server ready URLs are reported on (M5). */
export const TERMINAL_DEVSERVER_EVENT = 'terminal://devserver-ready'

/** A detected dev server, ready to preview (mirrors Rust `TerminalDevServerEvent`). */
export interface DevServerReady {
  id: string
  /** Browsable base URL, e.g. `http://localhost:5173`. */
  url: string
  /** The bound port parsed from the URL. */
  port: number
}

/**
 * Subscribe to the dev-server-ready event for one session. Fires at most once
 * per session (the backend detector latches). Returns the unlisten function;
 * events for other sessions are filtered out.
 */
export async function onDevServerReady(
  id: string,
  handler: (ready: DevServerReady) => void
): Promise<UnlistenFn> {
  return listen<DevServerReady>(TERMINAL_DEVSERVER_EVENT, (event) => {
    if (event.payload.id === id) handler(event.payload)
  })
}

/**
 * Subscribe to dev-server-ready events across ALL terminal sessions. Used by the
 * Web Preview controller, which is not session-scoped. Returns the unlisten
 * function.
 */
export async function onAnyDevServerReady(
  handler: (ready: DevServerReady) => void
): Promise<UnlistenFn> {
  return listen<DevServerReady>(TERMINAL_DEVSERVER_EVENT, (event) => handler(event.payload))
}

// ===========================================================================
// AI System (Milestone 4)
//
// Typed wrappers over the engine-backed `ai_*` / `conversation_*` commands and
// the `ai://chunk|done|error` streaming events. No provider HTTP or key handling
// lives here — that is all engine-side. API keys are write-only: `aiCheckKey`
// returns a bool, never the value.
// ===========================================================================

/** Normalized engine error (mirrors `gwenland_engine::ai::AiError`). Serde
 *  adjacently tagged: discriminate on `kind`. */
export type AiErrorKind =
  | 'key_not_set'
  | 'invalid_key'
  | 'rate_limit'
  | 'context_length_exceeded'
  | 'network'
  | 'provider_error'
  | 'cancelled'
  | 'keychain_error'
  | 'storage_error'

export interface AiError {
  kind: AiErrorKind
  /** `rate_limit` -> `{ retry_after }`; `network`/`provider_error`/
   *  `keychain_error`/`storage_error` -> string; unit variants omit it. */
  data?: { retry_after: number | null } | string
}

/** A model offered by a provider (mirrors `ModelInfo`). */
export interface ModelInfo {
  id: string
  display_name: string
}

/** Extra context attached to a prompt (mirrors `ContextAttachment`). Serde
 *  tagged by `type` with snake_case names. */
export type ContextAttachment =
  | { type: 'file'; path: string }
  | { type: 'selection'; path: string; content: string }
  | { type: 'terminal_error'; label: string; line: string }

/** Manifest metadata for one conversation (mirrors `ConversationMeta`). */
export interface ConversationMeta {
  id: string
  project_path: string
  jsonl_path: string
  title: string
  provider: string
  model: string
  created_at: string
  updated_at: string
  training_opt_in: boolean
}

/** One ChatML message in a persisted turn. */
export interface TurnMessage {
  role: string
  content: string
}

/** One persisted exchange (mirrors `ConversationTurn`). */
export interface ConversationTurn {
  messages: TurnMessage[]
  timestamp: string
  provider: string
  model: string
}

// --- Unified diff parsing (Milestone 8, Wave 5) ----------------------------
// Mirror the engine `DiffFile`/`DiffHunk`/`DiffLine` DTOs.

/** One line within a hunk (serde tag `kind`, content `text`). */
export type DiffLine =
  | { kind: 'context'; text: string }
  | { kind: 'added'; text: string }
  | { kind: 'removed'; text: string }

/** One hunk: old/new ranges plus its lines. */
export interface DiffHunk {
  old_start: number
  old_count: number
  new_start: number
  new_count: number
  header: string
  lines: DiffLine[]
}

/** One file's worth of hunks (paths null for /dev/null or missing headers). */
export interface DiffFile {
  old_path: string | null
  new_path: string | null
  hunks: DiffHunk[]
}

/** Parse assistant text into structured diff files. Rejects on malformed hunks
 *  with a stringified error the caller shows as a non-destructive notice. */
export function parseDiff(text: string): Promise<DiffFile[]> {
  return invoke<DiffFile[]>('parse_diff', { text })
}

// --- Engine settings (settings.toml `ai` section) --------------------------
// The UI manages provider/model preferences + generic providers + training
// opt-in here. NEVER contains API keys (those live in the keychain).

export interface GenericProviderSetting {
  id: string
  display_name: string
  base_url: string
  default_model: string
  extra_headers: Record<string, string>
}

export interface AiSettings {
  active_provider: string
  active_model: string
  training_opt_in: boolean
  generic_providers: GenericProviderSetting[]
}

/** Per-language server config (mirrors `LanguageServerSettings`). Empty
 *  `command` means "use the built-in default for this language". */
export interface LanguageServerSettings {
  enabled: boolean
  command: string
  args: string[]
}

/** The `[lsp]` settings section (mirrors `LspSettings`). TS/JS share the
 *  `typescript` bucket. Never stores secrets. */
export interface LspSettings {
  rust: LanguageServerSettings
  typescript: LanguageServerSettings
  python: LanguageServerSettings
}

export interface EngineSettings {
  version: number
  theme: { mode: string }
  ai: AiSettings
  lsp: LspSettings
}

/** Load the engine settings (settings.toml) including the `ai` section. */
export function loadEngineSettings(): Promise<EngineSettings> {
  return invoke<EngineSettings>('load_settings')
}

/** Persist the engine settings. */
export function saveEngineSettings(settings: EngineSettings): Promise<void> {
  return invoke<void>('save_settings', { settings })
}

// --- Key management --------------------------------------------------------

/** Store (or replace) an API key in the OS keychain. */
export function aiSetKey(provider: string, apiKey: string): Promise<void> {
  return invoke<void>('ai_set_key', { provider, apiKey })
}

/** Delete a provider's stored key (idempotent). */
export function aiDeleteKey(provider: string): Promise<void> {
  return invoke<void>('ai_delete_key', { provider })
}

/** Whether a key is stored for `provider` — never returns the value. */
export function aiCheckKey(provider: string): Promise<boolean> {
  return invoke<boolean>('ai_check_key', { provider })
}

/** List models for `provider`, or `null` when listing is unsupported. */
export function aiListModels(provider: string): Promise<ModelInfo[] | null> {
  return invoke<ModelInfo[] | null>('ai_list_models', { provider })
}

// --- Streaming -------------------------------------------------------------

/** An image attached to the current turn (mirrors engine `ImageAttachment`).
 *  `data` is base64 (no `data:` prefix). Not persisted to conversation JSONL. */
export interface ImageAttachment {
  mime: string
  data: string
}

export interface AiSendArgs {
  streamId: string
  conversationId: string
  message: string
  attachments: ContextAttachment[]
  images?: ImageAttachment[]
  provider?: string | null
  model?: string | null
  /** Per-workspace persona/system prompt prefix, layered over the base prompt
   *  (GWEN-334). Empty/omitted → engine default only. */
  systemPrefix?: string | null
}

/**
 * Start a streaming completion. Register the chunk/done/error listeners
 * (`onAiChunk`/`onAiDone`/`onAiError`) BEFORE calling this. Resolves with the
 * same `streamId` once the backend accepts the stream.
 */
export function aiSend(args: AiSendArgs): Promise<string> {
  return invoke<string>('ai_send', {
    streamId: args.streamId,
    conversationId: args.conversationId,
    message: args.message,
    attachments: args.attachments,
    images: args.images ?? [],
    provider: args.provider ?? null,
    model: args.model ?? null,
    systemPrefix: args.systemPrefix ?? null,
  })
}

/** Cancel an active stream. Missing/finished streams are a no-op success. */
export function aiCancel(streamId: string): Promise<void> {
  return invoke<void>('ai_cancel', { streamId })
}

/** M13 — deliver a web search result to a parked stream so generation resumes. */
export function aiSearchResult(streamId: string, resultText: string): Promise<void> {
  return invoke<void>('ai_search_result', { streamId, resultText })
}

/**
 * One-shot, non-streaming, non-persisted completion (GWEN-324). Used for short
 * side-prompts (e.g. auto-naming a conversation). Resolves to the trimmed
 * assistant text; rejects with a stringified error on failure. Does NOT touch
 * conversation history. Falls back to the active provider/model when omitted.
 */
export function aiComplete(
  prompt: string,
  provider?: string | null,
  model?: string | null
): Promise<string> {
  return invoke<string>('ai_complete', {
    prompt,
    provider: provider ?? null,
    model: model ?? null,
  })
}

// --- Conversations ---------------------------------------------------------

/** Create a new conversation under `projectRoot` and register it. */
export function conversationNew(
  projectRoot: string,
  title: string,
  provider: string,
  model: string
): Promise<ConversationMeta> {
  return invoke<ConversationMeta>('conversation_new', {
    projectRoot,
    title,
    provider,
    model,
  })
}

/** List conversations newest-first (stale entries skipped). */
export function conversationList(): Promise<ConversationMeta[]> {
  return invoke<ConversationMeta[]>('conversation_list')
}

/** Load a conversation's persisted turns. */
export function conversationLoad(conversationId: string): Promise<ConversationTurn[]> {
  return invoke<ConversationTurn[]>('conversation_load', { conversationId })
}

/** Rename a conversation (manifest title only). */
export function conversationRename(conversationId: string, title: string): Promise<void> {
  return invoke<void>('conversation_rename', { conversationId, title })
}

/**
 * Truncate a conversation to its first `keepCount` turns (GWEN-326 message
 * edit/rollback). Resolves to the surviving turns. Each turn is one
 * user+assistant exchange.
 */
export function conversationTruncate(
  conversationId: string,
  keepCount: number
): Promise<ConversationTurn[]> {
  return invoke<ConversationTurn[]>('conversation_truncate', { conversationId, keepCount })
}

/** Delete a conversation (JSONL + manifest entry). */
export function conversationDelete(conversationId: string): Promise<void> {
  return invoke<void>('conversation_delete', { conversationId })
}

/** Set a conversation's training opt-in flag (explicit user action only). */
export function conversationSetTrainingOptIn(
  conversationId: string,
  optIn: boolean
): Promise<void> {
  return invoke<void>('conversation_set_training_opt_in', { conversationId, optIn })
}

// --- Streaming event listeners ---------------------------------------------

export const AI_CHUNK_EVENT = 'ai://chunk'
export const AI_DONE_EVENT = 'ai://done'
export const AI_ERROR_EVENT = 'ai://error'

interface AiChunkPayload {
  stream_id: string
  text: string
}
interface AiDonePayload {
  stream_id: string
}
interface AiErrorPayload {
  stream_id: string
  error: AiError
}

/** Subscribe to streamed tokens for `streamId`. Other streams are filtered out. */
export async function onAiChunk(
  streamId: string,
  handler: (text: string) => void
): Promise<UnlistenFn> {
  return listen<AiChunkPayload>(AI_CHUNK_EVENT, (event) => {
    if (event.payload.stream_id === streamId) handler(event.payload.text)
  })
}

/** Subscribe to successful completion of `streamId` (terminal). */
export async function onAiDone(streamId: string, handler: () => void): Promise<UnlistenFn> {
  return listen<AiDonePayload>(AI_DONE_EVENT, (event) => {
    if (event.payload.stream_id === streamId) handler()
  })
}

/** Subscribe to error/cancellation of `streamId` (terminal). */
export async function onAiError(
  streamId: string,
  handler: (error: AiError) => void
): Promise<UnlistenFn> {
  return listen<AiErrorPayload>(AI_ERROR_EVENT, (event) => {
    if (event.payload.stream_id === streamId) handler(event.payload.error)
  })
}

/** Human-readable, key-free message for an `AiError`. */
export function aiErrorMessage(error: AiError): string {
  switch (error.kind) {
    case 'key_not_set':
      return 'No API key is set for this provider. Add one in Settings.'
    case 'invalid_key':
      return 'The API key was rejected. Check it in Settings.'
    case 'rate_limit': {
      const retry =
        typeof error.data === 'object' && error.data?.retry_after
          ? ` Try again in ${error.data.retry_after}s.`
          : ''
      return `Rate limited by the provider.${retry}`
    }
    case 'context_length_exceeded':
      return 'The request exceeded the model context length. Remove some context and retry.'
    case 'network':
      return 'Network error reaching the provider. Check your connection.'
    case 'cancelled':
      return 'Request cancelled.'
    case 'keychain_error':
      return 'Could not access the OS keychain.'
    case 'storage_error':
      return 'Could not read or write conversation history.'
    case 'provider_error':
    default:
      return typeof error.data === 'string' ? error.data : 'The provider returned an error.'
  }
}

// ===========================================================================
// Git integration (Wave 2 — GWEN-327..331)
//
// Typed wrappers over the engine-backed `git_*` commands, which shell out to the
// system `git` binary. Every call takes the workspace `root`.
// ===========================================================================

/** One changed file (mirrors `gwenland_engine::git::GitFileStatus`). */
export interface GitFileStatus {
  /** Repo-relative path (forward slashes). */
  path: string
  /** Single-letter badge: M/A/D/U/R/C. */
  status: string
  /** Whether the change (or part of it) is staged. */
  staged: boolean
  /** Whether the file is untracked. */
  untracked: boolean
}

/** Branch + dirty summary + file list (mirrors `gwenland_engine::git::GitStatus`). */
export interface GitStatus {
  branch: string
  dirty_count: number
  files: GitFileStatus[]
  /** Commits ahead of the upstream tracking branch (0 when no upstream). */
  ahead: number
  /** Commits behind the upstream tracking branch (0 when no upstream). */
  behind: number
}

/** Whether `root` is inside a git work tree. */
export function gitIsRepo(root: string): Promise<boolean> {
  return invoke<boolean>('git_is_repo', { root })
}

/** Full status snapshot (branch, dirty count, per-file list). */
export function gitStatus(root: string): Promise<GitStatus> {
  return invoke<GitStatus>('git_status', { root })
}

/** Bounded, read-only commit graph payload for the Canvas2D graph view. */
export function getGitGraph(
  workspacePath: string,
  maxCommits: number | null = 300
): Promise<CommitGraphPayload> {
  return invoke<CommitGraphPayload>('get_git_graph', { workspacePath, maxCommits })
}

/** Lazy, read-only metadata for one commit. */
export function getCommitDetails(workspacePath: string, hash: string): Promise<CommitDetails> {
  return invoke<CommitDetails>('get_commit_details', { workspacePath, hash })
}

/** Lazy, read-only unified diff for one commit. */
export function getCommitDiff(workspacePath: string, hash: string): Promise<string> {
  return invoke<string>('get_commit_diff', { workspacePath, hash })
}

/** Stage one path, or everything when `all` is true. */
export function gitStage(root: string, path: string, all = false): Promise<void> {
  return invoke<void>('git_stage', { root, path, all })
}

/** Unstage one path, or everything when `all` is true. */
export function gitUnstage(root: string, path: string, all = false): Promise<void> {
  return invoke<void>('git_unstage', { root, path, all })
}

/** Discard local changes to a path (deletes untracked files). */
export function gitDiscard(root: string, path: string, untracked: boolean): Promise<void> {
  return invoke<void>('git_discard', { root, path, untracked })
}

/** Commit the staged index with `message`. */
export function gitCommit(root: string, message: string): Promise<void> {
  return invoke<void>('git_commit', { root, message })
}

/** Push the current branch. Resolves to git's output. */
export function gitPush(root: string): Promise<string> {
  return invoke<string>('git_push', { root })
}

/** Pull. Resolves to git's output. */
export function gitPull(root: string): Promise<string> {
  return invoke<string>('git_pull', { root })
}

/** Unified diff for one path (synthesized for untracked files). */
export function gitDiffFile(root: string, path: string, untracked: boolean): Promise<string> {
  return invoke<string>('git_diff_file', { root, path, untracked })
}

/** All local branch names. */
export function gitListBranches(root: string): Promise<string[]> {
  return invoke<string[]>('git_list_branches', { root })
}

/** Switch to an existing branch. */
export function gitCheckout(root: string, branch: string): Promise<void> {
  return invoke<void>('git_checkout', { root, branch })
}

/** Create + switch to a new branch (name is slugified). Resolves to the slug. */
export function gitCreateBranch(root: string, name: string): Promise<string> {
  return invoke<string>('git_create_branch', { root, name })
}

/** Delete a local branch. */
export function gitDeleteBranch(root: string, branch: string): Promise<void> {
  return invoke<void>('git_delete_branch', { root, branch })
}

// ===========================================================================
// LSP Bridge (Milestone 6)
//
// Typed wrappers over the engine-backed `lsp_*` commands and the
// `lsp://diagnostics` / `lsp://status` events. No protocol parsing lives here —
// that is all engine-side. Versions are generated by the UI (the editor wiring)
// and passed through so Rust never invents conflicting versions.
// ===========================================================================

/** LSP language ids M6 understands (mirrors `gwenland_engine::lsp::LanguageId`). */
export type LspLanguage = 'rust' | 'typescript' | 'javascript' | 'python'

/** Per-file LSP status (mirrors `LspStatus`, serde tagged by `state`). */
export type LspStatus =
  | { state: 'plain_text' }
  | { state: 'unsupported_language' }
  | { state: 'disabled'; language: LspLanguage }
  | { state: 'missing_server'; language: LspLanguage; command: string }
  | { state: 'starting'; language: LspLanguage }
  | { state: 'connected'; language: LspLanguage; server_name: string | null }
  | { state: 'crashed'; language: LspLanguage; message: string }

/** UI diagnostic severity (mirrors `DiagnosticSeverity`). */
export type LspDiagnosticSeverity = 'error' | 'warning' | 'information' | 'hint'

/** Zero-based LSP range (UTF-16 code units; converted to CM offsets in Wave 4). */
export interface LspRange {
  start_line: number
  start_character: number
  end_line: number
  end_character: number
}

/** One normalized diagnostic (mirrors `LspDiagnostic`). */
export interface LspDiagnostic {
  range: LspRange
  severity: LspDiagnosticSeverity
  /** Omitted by the engine when absent. */
  source?: string | null
  code?: string | null
  message: string
}

// --- Commands --------------------------------------------------------------

/** Current LSP status for `path` (does not spawn a server). */
export function lspStatus(path: string): Promise<LspStatus> {
  return invoke<LspStatus>('lsp_status', { path })
}

/**
 * Open an eligible document for LSP. Ensures the server and sends `didOpen`.
 * `workspaceRoot` (the open project folder) refines workspace-root detection.
 * Resolves to the resulting status; unsupported/disabled/missing/crashed are
 * non-blocking states, not rejections.
 */
export function lspOpenDocument(
  path: string,
  text: string,
  version: number,
  workspaceRoot?: string | null
): Promise<LspStatus> {
  return invoke<LspStatus>('lsp_open_document', {
    path,
    text,
    version,
    workspaceRoot: workspaceRoot ?? null,
  })
}

/** Push a full-text change. No-op server-side when the doc isn't LSP-backed. */
export function lspChangeDocument(path: string, text: string, version: number): Promise<void> {
  return invoke<void>('lsp_change_document', { path, text, version })
}

/** Close an LSP-backed document (sends `didClose`, clears its diagnostics). */
export function lspCloseDocument(path: string): Promise<void> {
  return invoke<void>('lsp_close_document', { path })
}

/** Manually restart a server bucket (`'rust' | 'typescript' | 'python'`). */
export function lspRestart(language: string): Promise<LspStatus> {
  return invoke<LspStatus>('lsp_restart', { language })
}

/** One normalized completion option (mirrors `LspCompletionOption`). */
export interface LspCompletionOption {
  label: string
  detail?: string | null
  documentation?: string | null
  insert_text: string
  kind?: string | null
}

/**
 * Request completions at a zero-based `line`/`character` (UTF-16). Always
 * resolves (never rejects): an empty list means no options / missing server /
 * timeout, so the autocomplete source never disrupts typing.
 */
export function lspCompletion(
  path: string,
  line: number,
  character: number,
  version: number
): Promise<LspCompletionOption[]> {
  return invoke<LspCompletionOption[]>('lsp_completion', { path, line, character, version })
}

export interface LspDefinitionLocation {
  path: string
  line: number
  character: number
}

export function lspDefinition(
  path: string,
  line: number,
  character: number,
  version: number
): Promise<LspDefinitionLocation | null> {
  return invoke<LspDefinitionLocation | null>('lsp_definition', {
    path,
    line,
    character,
    version,
  })
}

export interface LspHover {
  contents: string
}

export function lspHover(
  path: string,
  line: number,
  character: number,
  version: number
): Promise<LspHover | null> {
  return invoke<LspHover | null>('lsp_hover', {
    path,
    line,
    character,
    version,
  })
}

// --- Events ----------------------------------------------------------------

export const LSP_DIAGNOSTICS_EVENT = 'lsp://diagnostics'
export const LSP_STATUS_EVENT = 'lsp://status'
export const WORKSPACE_SEARCH_RESULT_EVENT = 'search:result'
export const WORKSPACE_SEARCH_DONE_EVENT = 'search:done'

/** `lsp://diagnostics` payload (mirrors `DiagnosticsUpdate`). */
export interface LspDiagnosticsEvent {
  uri: string
  path: string
  language: LspLanguage
  workspace_root: string
  diagnostics: LspDiagnostic[]
}

/** `lsp://status` payload (mirrors `StatusUpdate`: status fields flattened plus
 *  `language`/`workspace_root`). */
export type LspStatusEvent = LspStatus & {
  language: LspLanguage | null
  workspace_root: string | null
}

/**
 * Subscribe to diagnostics updates across all documents. The handler routes by
 * `path`/`uri`. Returns the unlisten function.
 */
export async function onLspDiagnostics(
  handler: (event: LspDiagnosticsEvent) => void
): Promise<UnlistenFn> {
  return listen<LspDiagnosticsEvent>(LSP_DIAGNOSTICS_EVENT, (event) => handler(event.payload))
}

/**
 * Subscribe to LSP status transitions (connected/crashed/restart/…). The
 * handler routes by `language`/`workspace_root`. Returns the unlisten function.
 */
export async function onLspStatus(
  handler: (event: LspStatusEvent) => void
): Promise<UnlistenFn> {
  return listen<LspStatusEvent>(LSP_STATUS_EVENT, (event) => handler(event.payload))
}

export async function onWorkspaceSearchResult(
  handler: (event: WorkspaceSearchResultEvent) => void
): Promise<UnlistenFn> {
  return listen<WorkspaceSearchResultEvent>(WORKSPACE_SEARCH_RESULT_EVENT, (event) => handler(event.payload))
}

export async function onWorkspaceSearchDone(
  handler: (event: WorkspaceSearchDoneEvent) => void
): Promise<UnlistenFn> {
  return listen<WorkspaceSearchDoneEvent>(WORKSPACE_SEARCH_DONE_EVENT, (event) => handler(event.payload))
}

// ===========================================================================
// Agentic Coding Workflow (Milestone 10)
//
// Typed wrappers over the engine-backed `agent_*` commands and the
// `agent://chunk|phase|error` events. These mirror the engine `agentic` DTOs
// (serde snake_case). No provider HTTP or key handling lives here — sessions
// carry only provider/model ids; keys stay OS-keychain-only.
// ===========================================================================

/** Phases of the human-gated loop (mirrors `AgentPhase`, serde snake_case). */
export type AgentPhase =
  | 'goal'
  | 'drafting_plan'
  | 'awaiting_plan_approval'
  | 'drafting_edits'
  | 'awaiting_edit_approval'
  | 'applying_approved_edits'
  | 'awaiting_validation_approval'
  | 'validating'
  | 'summarizing'
  | 'complete'
  | 'failed'
  | 'cancelled'

/** Conservative command risk classification (mirrors `CommandRisk`). */
export type CommandRisk =
  | 'safe_check'
  | 'dependency_changing'
  | 'file_mutating'
  | 'destructive'
  | 'blocked'

/** A proposed validation command (mirrors `ValidationCommand`). */
export interface ValidationCommand {
  id: string
  command: string
  cwd: string
  reason: string
  risk: CommandRisk
  size_impact_note: string | null
}

/** A captured validation run (mirrors `ValidationRun`). */
export interface ValidationRun {
  id: string
  command_id: string
  status: 'pending' | 'running' | 'passed' | 'failed' | 'blocked' | 'cancelled'
  exit_code: number | null
  output_excerpt: string
  started_at: string
  finished_at: string | null
}

export type PlanStepStatus = 'pending' | 'in_progress' | 'done' | 'skipped'

/** One plan step (mirrors `PlanStep`). */
export interface PlanStep {
  id: string
  label: string
  description: string
  status: PlanStepStatus
}

/** A model-produced plan (mirrors `AgentPlan`). */
export interface AgentPlan {
  id: string
  title: string
  assumptions: string[]
  steps: PlanStep[]
  likely_files: string[]
  risks: string[]
  suggested_validation: ValidationCommand[]
  missing_context: string[]
}

/** What an approval unlocks (mirrors `ApprovalKind`). */
export type ApprovalKind = 'plan' | 'edits' | 'validation_command'

/** A one-use, session-scoped approval token (mirrors `ApprovalRecord`). */
export interface ApprovalRecord {
  id: string
  kind: ApprovalKind
  target_id: string
  created_at: string
  consumed: boolean
}

/** What a context item represents (mirrors `ContextItemKind`). */
export type ContextItemKind =
  | 'active_file'
  | 'selection'
  | 'open_tab'
  | 'diagnostic'
  | 'terminal_error'
  | 'file'
  | 'workspace_tree'

/** One candidate context item (mirrors `ContextItem`). */
export interface ContextItem {
  id: string
  kind: ContextItemKind
  path: string | null
  label: string
  content: string | null
  byte_len: number
  included: boolean
  redacted: boolean
  reason: string
}

/** Why a candidate was omitted (mirrors `OmissionReason`). */
export type OmissionReason =
  | 'secret_path'
  | 'excluded'
  | 'oversized'
  | 'binary'
  | 'read_error'
  | 'outside_workspace'
  | 'user_removed'

/** An omitted candidate with a user-safe reason (mirrors `ContextOmission`). */
export interface ContextOmission {
  path: string
  label: string
  reason: OmissionReason
  detail: string
}

/** The context preview shown before a provider request (mirrors `ContextPreview`). */
export interface ContextPreview {
  items: ContextItem[]
  total_bytes: number
  omitted: ContextOmission[]
}

/** Per-file/hunk approval state (mirrors `ApprovalState`). */
export type ApprovalState = 'pending' | 'approved' | 'rejected' | 'failed'

/** File-level change kind (mirrors `FileChangeKind`). */
export type FileChangeKind = 'modify' | 'create' | 'delete' | 'rename'

/** One reviewable hunk (mirrors `ProposedHunk`). Reuses M8 `DiffLine`. */
export interface ProposedHunk {
  id: string
  old_start: number
  old_count: number
  new_start: number
  new_count: number
  header: string
  lines: DiffLine[]
  approval: ApprovalState
}

/** One file's proposed change (mirrors `ProposedFileChange`). */
export interface ProposedFileChange {
  id: string
  old_path: string | null
  new_path: string | null
  change_kind: FileChangeKind
  hunks: ProposedHunk[]
  approval: ApprovalState
}

/** A reviewable set of proposed changes (mirrors `ChangeSet`). */
export interface ChangeSet {
  id: string
  plan_id: string
  files: ProposedFileChange[]
  parse_warnings: string[]
}

/** One file's apply outcome (mirrors `ApplyOutcome`). */
export interface ApplyOutcome {
  file_id: string
  path: string
  hunk_ids: string[]
  message: string
}

/** Result of an apply pass (mirrors `ApplyReport`). */
export interface ApplyReport {
  applied: ApplyOutcome[]
  rejected: ApplyOutcome[]
  skipped: ApplyOutcome[]
  failed: ApplyOutcome[]
}

/** The final session summary (mirrors `AgentSummary`). */
export interface AgentSummary {
  id: string
  goal: string
  plan_title: string
  changed_files: string[]
  applied_count: number
  rejected_count: number
  failed_count: number
  skipped_count: number
  validation_status: string
  unresolved_risks: string[]
  follow_ups: string[]
  text: string
  local_fallback: boolean
}

/** One agent session (mirrors `AgentSession`). Holds no keys/headers. */
/** Autonomy tier governing how Wave-7 gates are satisfied (M10 Wave 8). */
export type AgentTier = 'ask' | 'accept_for_me' | 'full_control'

export interface AgentSession {
  id: string
  project_root: string
  goal: string
  phase: AgentPhase
  interrupted: boolean
  provider: string
  model: string
  tier: AgentTier
  context: ContextPreview
  plan: AgentPlan | null
  approvals: ApprovalRecord[]
  change_sets: ChangeSet[]
  apply_report: ApplyReport | null
  validation_runs: ValidationRun[]
  summary: AgentSummary | null
}

/** Workspace state offered as candidate context (mirrors `AgentContextInput`). */
export interface AgentContextInput {
  active_file?: string | null
  selection?: { path: string; content: string } | null
  open_tabs?: string[]
}

// --- Commands --------------------------------------------------------------

/** Create an agent session in the `goal` phase. Provider/model fall back to the
 *  global active settings when omitted. */
export function agentCreateSession(
  projectRoot: string,
  goal: string,
  provider?: string | null,
  model?: string | null,
  tier?: AgentTier | null
): Promise<AgentSession> {
  return invoke<AgentSession>('agent_create_session', {
    projectRoot,
    goal,
    provider: provider ?? null,
    model: model ?? null,
    tier: tier ?? null,
  })
}

/** Change a session's autonomy tier (allowed only between iterations). */
export function agentSetTier(sessionId: string, tier: AgentTier): Promise<AgentSession> {
  return invoke<AgentSession>('agent_set_tier', { sessionId, tier })
}

/** Build + store a policy-filtered context preview for a session. */
export function agentContextPreview(
  sessionId: string,
  input: AgentContextInput
): Promise<ContextPreview> {
  return invoke<ContextPreview>('agent_context_preview', { sessionId, input })
}

/**
 * Request a plan. Register the `agent://*` listeners BEFORE calling this.
 * `contextItemIds` selects which preview items to include (empty = all
 * currently-included items). Resolves with the same `streamId` once accepted.
 */
export function agentRequestPlan(
  sessionId: string,
  streamId: string,
  contextItemIds: string[]
): Promise<string> {
  return invoke<string>('agent_request_plan', { sessionId, streamId, contextItemIds })
}

/**
 * Request edit proposals for an approved plan. Register the `agent://*`
 * listeners BEFORE calling this. Resolves with the same stream id once accepted.
 */
export function agentRequestEdits(
  sessionId: string,
  streamId: string,
  contextItemIds: string[]
): Promise<string> {
  return invoke<string>('agent_request_edits', { sessionId, streamId, contextItemIds })
}

/** Approve the current plan, unlocking edit generation. Resolves to the record. */
export function agentApprovePlan(sessionId: string, planId: string): Promise<ApprovalRecord> {
  return invoke<ApprovalRecord>('agent_approve_plan', { sessionId, planId })
}

/** Read-only session snapshot (phase, plan, context, approvals, …). */
/** Set one hunk's review approval state. Does not write files. */
export function agentSetHunkApproval(
  sessionId: string,
  changeSetId: string,
  hunkId: string,
  approval: ApprovalState
): Promise<AgentSession> {
  return invoke<AgentSession>('agent_set_hunk_approval', {
    sessionId,
    changeSetId,
    hunkId,
    approval,
  })
}

/** Set every hunk in one file change to a review approval state. No file writes. */
export function agentSetFileApproval(
  sessionId: string,
  changeSetId: string,
  fileId: string,
  approval: ApprovalState
): Promise<AgentSession> {
  return invoke<AgentSession>('agent_set_file_approval', {
    sessionId,
    changeSetId,
    fileId,
    approval,
  })
}

/** Apply approved hunks/files. Destructive changes require explicit confirmation. */
export function agentApplyChanges(
  sessionId: string,
  destructiveConfirmed: boolean
): Promise<AgentSession> {
  return invoke<AgentSession>('agent_apply_changes', {
    sessionId,
    destructiveConfirmed,
  })
}

/** Approve one validation command after risk review. Does not run it. */
export function agentApproveValidationCommand(
  sessionId: string,
  commandId: string,
  sizeImpactNote: string | null,
  dangerConfirmed: boolean
): Promise<ApprovalRecord> {
  return invoke<ApprovalRecord>('agent_approve_validation_command', {
    sessionId,
    commandId,
    sizeImpactNote,
    dangerConfirmed,
  })
}

/** Run one approved validation command and return the updated session. */
export function agentRunValidation(
  sessionId: string,
  commandId: string,
  approvalId: string
): Promise<AgentSession> {
  return invoke<AgentSession>('agent_run_validation', { sessionId, commandId, approvalId })
}

/** Build the final deterministic session summary and mark the session complete. */
export function agentSummarize(sessionId: string): Promise<AgentSession> {
  return invoke<AgentSession>('agent_summarize', { sessionId })
}

/** Restore persisted agent sessions for a project. Runtime stream handles are not restored. */
export function agentRestoreSessions(projectRoot?: string | null): Promise<AgentSession[]> {
  return invoke<AgentSession[]>('agent_restore_sessions', {
    projectRoot: projectRoot ?? null,
  })
}

export function agentGetSession(sessionId: string): Promise<AgentSession> {
  return invoke<AgentSession>('agent_get_session', { sessionId })
}

/** Cancel a session (aborts any active stream, moves it to `cancelled`). */
export function agentCancel(sessionId: string): Promise<void> {
  return invoke<void>('agent_cancel', { sessionId })
}

// --- Events ----------------------------------------------------------------

export const AGENT_CHUNK_EVENT = 'agent://chunk'
export const AGENT_PHASE_EVENT = 'agent://phase'
export const AGENT_ERROR_EVENT = 'agent://error'

interface AgentChunkPayload {
  stream_id: string
  text: string
}
interface AgentPhasePayload {
  session_id: string
  phase: AgentPhase
}
interface AgentErrorPayload {
  session_id: string
  stream_id: string
  error: AiError
}

/** Subscribe to streamed tokens for `streamId`. Other streams are filtered out. */
export async function onAgentChunk(
  streamId: string,
  handler: (text: string) => void
): Promise<UnlistenFn> {
  return listen<AgentChunkPayload>(AGENT_CHUNK_EVENT, (event) => {
    if (event.payload.stream_id === streamId) handler(event.payload.text)
  })
}

/** Subscribe to phase transitions for `sessionId`. Other sessions filtered out. */
export async function onAgentPhase(
  sessionId: string,
  handler: (phase: AgentPhase) => void
): Promise<UnlistenFn> {
  return listen<AgentPhasePayload>(AGENT_PHASE_EVENT, (event) => {
    if (event.payload.session_id === sessionId) handler(event.payload.phase)
  })
}

/** Subscribe to recoverable errors for `sessionId`. Other sessions filtered out. */
export async function onAgentError(
  sessionId: string,
  handler: (error: AiError, streamId: string) => void
): Promise<UnlistenFn> {
  return listen<AgentErrorPayload>(AGENT_ERROR_EVENT, (event) => {
    if (event.payload.session_id === sessionId) {
      handler(event.payload.error, event.payload.stream_id)
    }
  })
}

// ===========================================================================
// Agent tool-calling ReAct loop (M10 Wave 7)
//
// `agentToolStep` runs one provider turn server-side: it auto-runs non-gated
// tools (read/git/diagnostics/open_browser) and parks mutating/terminal/ask
// tools as `awaiting` so the UI can gate them via `agentToolResolve`. Tool
// activity streams over `agent://tool_call|tool_result|ask`.
// ===========================================================================

/** Outcome of one `agent_tool_step` (mirrors the Rust `AgentStepResult`). */
export type AgentStepResult =
  | { kind: 'final'; text: string }
  | { kind: 'exhausted' }
  | { kind: 'ran'; tool: string; ok: boolean }
  | { kind: 'awaiting'; id: string; tool: string; side: string; risk: string | null }

/** Run one ReAct step. Register `onAgent*` listeners BEFORE calling. */
export function agentToolStep(
  sessionId: string,
  streamId: string,
  contextItemIds: string[]
): Promise<AgentStepResult> {
  return invoke<AgentStepResult>('agent_tool_step', { sessionId, streamId, contextItemIds })
}

/** Resolve a parked gated tool. `decision`: "approve" | "confirm" | "reject".
 *  `selection` carries the chosen option(s) for ask_user (empty otherwise). */
export function agentToolResolve(
  sessionId: string,
  decision: 'approve' | 'confirm' | 'reject',
  selection: string[] = []
): Promise<void> {
  return invoke<void>('agent_tool_resolve', { sessionId, decision, selection })
}

/** Open an http/https URL in the OS browser (also the agent's open_browser tool). */
export function openBrowser(url: string): Promise<void> {
  return invoke<void>('open_browser', { url })
}

export const AGENT_TOOL_CALL_EVENT = 'agent://tool_call'
export const AGENT_TOOL_RESULT_EVENT = 'agent://tool_result'
export const AGENT_ASK_EVENT = 'agent://ask'

export interface AgentToolCallPayload {
  session_id: string
  id: string
  tool: string
  /** JSON-encoded args string. */
  args: string
}
export interface AgentToolResultPayload {
  session_id: string
  id: string
  ok: boolean
  content: string
  error: string | null
}
export interface AgentAskPayload {
  session_id: string
  id: string
  prompt: string
  options: string[]
  multi: boolean
}

/** Subscribe to tool-call announcements for `sessionId`. */
export async function onAgentToolCall(
  sessionId: string,
  handler: (call: AgentToolCallPayload) => void
): Promise<UnlistenFn> {
  return listen<AgentToolCallPayload>(AGENT_TOOL_CALL_EVENT, (event) => {
    if (event.payload.session_id === sessionId) handler(event.payload)
  })
}

/** Subscribe to tool observations for `sessionId`. */
export async function onAgentToolResult(
  sessionId: string,
  handler: (result: AgentToolResultPayload) => void
): Promise<UnlistenFn> {
  return listen<AgentToolResultPayload>(AGENT_TOOL_RESULT_EVENT, (event) => {
    if (event.payload.session_id === sessionId) handler(event.payload)
  })
}

/** Subscribe to ask_user prompts for `sessionId`. */
export async function onAgentAsk(
  sessionId: string,
  handler: (ask: AgentAskPayload) => void
): Promise<UnlistenFn> {
  return listen<AgentAskPayload>(AGENT_ASK_EVENT, (event) => {
    if (event.payload.session_id === sessionId) handler(event.payload)
  })
}

// ===========================================================================
// Agent terminal streaming (anti-freeze fix)
//
// When an agent run_terminal_cmd executes (npm install, npx create-next-app,
// etc.), the Rust side streams stdout/stderr lines as `agent://cmd_output`
// events and emits `agent://cmd_done` on process exit. The UI listens to these
// to show live progress and a kill button without blocking the webview thread.
// ===========================================================================

export const AGENT_CMD_OUTPUT_EVENT = 'agent://cmd_output'
export const AGENT_CMD_DONE_EVENT = 'agent://cmd_done'

export interface AgentCmdOutputPayload {
  session_id: string
  line: string
}

export interface AgentCmdDonePayload {
  session_id: string
  success: boolean
  output: string
}

/** Subscribe to live stdout/stderr lines for one session's running command. */
export async function onAgentCmdOutput(
  sessionId: string,
  handler: (line: string) => void
): Promise<UnlistenFn> {
  return listen<AgentCmdOutputPayload>(AGENT_CMD_OUTPUT_EVENT, (event) => {
    if (event.payload.session_id === sessionId) handler(event.payload.line)
  })
}

/** Subscribe to command completion for one session. */
export async function onAgentCmdDone(
  sessionId: string,
  handler: (payload: AgentCmdDonePayload) => void
): Promise<UnlistenFn> {
  return listen<AgentCmdDonePayload>(AGENT_CMD_DONE_EVENT, (event) => {
    if (event.payload.session_id === sessionId) handler(event.payload)
  })
}

/**
 * Kill the child process currently running an agent terminal command.
 * Best-effort: if no command is running for this session, it's a no-op.
 */
export function agentKillTerminal(sessionId: string): Promise<void> {
  return invoke<void>('agent_kill_terminal', { sessionId })
}

// ===========================================================================
// Workspace Settings (M14 Wave 1)
//
// Per-workspace settings overlay stored under `.gwenland/settings.json`.
// These merge on top of global settings; absent fields (null) fall back to
// global values. No API keys, tokens, or secrets may appear here.
// ===========================================================================

/** Safety strictness level (mirrors `gwenland_engine::workspace::SafetyStrictness`). */
export type SafetyStrictness = 'standard' | 'strict' | 'paranoid'

/**
 * Per-workspace settings overlay (mirrors `WorkspaceSettings`).
 * Every field is optional (`null` = inherit from global settings).
 * **Must never contain API keys, tokens, passwords, or credentials.**
 */
export interface WorkspaceSettings {
  theme?: string | null
  accent_color?: string | null
  editor_font?: string | null
  terminal_font?: string | null
  last_terminal_shell?: string | null
  layout_state?: unknown | null
  sidebar_open?: boolean | null
  panel_open?: boolean | null
  keybindings?: unknown | null
  formatter?: string | null
  autosave?: boolean | null
  safety_strictness?: SafetyStrictness | null
}

/**
 * Load the per-workspace settings overlay from `.gwenland/settings.json`.
 * Always resolves — returns an all-null object when the file is absent or
 * malformed, never rejects on missing config.
 */
export function workspaceLoadSettings(workspaceRoot: string): Promise<WorkspaceSettings> {
  return invoke<WorkspaceSettings>('workspace_load_settings', { workspaceRoot })
}

/**
 * Save the per-workspace settings overlay to `.gwenland/settings.json`.
 * Creates `.gwenland/` if needed; write is atomic (tmp + rename) engine-side.
 */
export function workspaceSaveSettings(
  workspaceRoot: string,
  settings: WorkspaceSettings
): Promise<void> {
  return invoke<void>('workspace_save_settings', { workspaceRoot, settings })
}

// ===========================================================================
// Workspace/Layout Restore State (M15 / GWEN-348)
//
// UI-owned JSON blobs stored under `.gwenland/workspace.json` and
// `.gwenland/layout.json`. Missing or malformed files resolve to null.
// ===========================================================================

export interface PersistedWorkspaceTab {
  path: string
  type: string
  isDirty: boolean
  isPreview?: boolean
}

export interface PersistedConversationState {
  isOpen?: boolean
  activeConversationId?: string | null
  activeProvider?: string
  activeModel?: string
  reasoningLevel?: string
  unsentInput?: string
}

export interface PersistedWorkspaceState {
  workspaceRoot: string
  openTabs: PersistedWorkspaceTab[]
  activeTabPath: string
  conversationState?: PersistedConversationState | null
}

export interface PersistedLayoutState {
  sidebarOpen: boolean
  sidebarWidth: number
  bottomPanelOpen: boolean
  bottomPanelHeight: number
  terminalOpen: boolean
  theme: string
  editorGroupOrientation?: 'horizontal' | 'vertical'
  activeEditorGroupId?: string
  editorGroups?: PersistedEditorGroup[]
}

export interface PersistedEditorGroup {
  id: string
  tabs: PersistedWorkspaceTab[]
  activeTabPath: string
  isLocked: boolean
  isMaximized: boolean
  size?: number
}

export function loadWorkspaceState(
  workspaceRoot: string
): Promise<PersistedWorkspaceState | null> {
  return invoke<PersistedWorkspaceState | null>('load_workspace_state', { workspaceRoot })
}

export function saveWorkspaceState(
  workspaceRoot: string,
  state: PersistedWorkspaceState
): Promise<void> {
  return invoke<void>('save_workspace_state', { workspaceRoot, state })
}

export function loadLayoutState(workspaceRoot: string): Promise<PersistedLayoutState | null> {
  return invoke<PersistedLayoutState | null>('load_layout_state', { workspaceRoot })
}

export function saveLayoutState(
  workspaceRoot: string,
  state: PersistedLayoutState
): Promise<void> {
  return invoke<void>('save_layout_state', { workspaceRoot, state })
}

// ===========================================================================
// Local File History (M16 / GWEN-354)
// ===========================================================================

export type HistorySource = 'save' | 'manual' | 'ai'

export interface HistoryEntry {
  timestamp: string
  size: number
  source: HistorySource | string
}

export function historySaveEntry(
  workspaceRoot: string,
  filePath: string,
  content: string,
  source: HistorySource
): Promise<HistoryEntry | null> {
  return invoke<HistoryEntry | null>('history_save_entry', {
    workspaceRoot,
    filePath,
    content,
    source
  })
}

export function historyList(
  workspaceRoot: string,
  filePath: string
): Promise<HistoryEntry[]> {
  return invoke<HistoryEntry[]>('history_list', { workspaceRoot, filePath })
}

export function historyReadEntry(
  workspaceRoot: string,
  filePath: string,
  timestamp: string
): Promise<string> {
  return invoke<string>('history_read_entry', { workspaceRoot, filePath, timestamp })
}

export function historyClear(workspaceRoot: string, filePath: string): Promise<void> {
  return invoke<void>('history_clear', { workspaceRoot, filePath })
}

// ===========================================================================
// Safety Engine (M14 Wave 5)
//
// Typed wrappers over the local safety evaluation command and search policy
// helper. The engine owns all policy logic; these are thin call bridges.
// ===========================================================================

/** Risk level for a safety decision (mirrors `RiskLevel`). */
export type RiskLevel =
  | 'safe'
  | 'low'
  | 'medium'
  | 'high'
  | 'destructive'
  | 'secret'
  | 'remote'
  | 'unknown'

/** Safety verdict (mirrors `SafetyVerdict`). */
export type SafetyVerdict = 'allow' | 'ask' | 'block'

/** Confirmation kind (mirrors `ConfirmationKind`). */
export type ConfirmationKind =
  | { kind: 'none' }
  | { kind: 'simple' }
  | { kind: 'typed' }
  | { kind: 'danger_ack'; warning: string }

/** Safety decision returned by the engine (mirrors `SafetyDecision`). */
export interface SafetyDecision {
  action_id: string
  verdict: SafetyVerdict
  risk: RiskLevel
  reason: string
  confirmation: ConfirmationKind
  protected_path_matched: boolean
  secret_path_matched: boolean
}

/**
 * Evaluate a safety action. `actionKindJson` is the JSON-serialized
 * `SafetyActionKind` (use `JSON.stringify({ kind: "file_write", path: "…" })`).
 * Returns the local policy decision — no network call is made.
 */
export function safetyEvaluate(
  actionKindJson: string,
  workspaceRoot: string,
  actor: 'user' | 'agent' | 'system' | string,
  strictness: SafetyStrictness
): Promise<SafetyDecision> {
  return invoke<SafetyDecision>('safety_evaluate', {
    actionKindJson,
    workspaceRoot,
    actor,
    strictness
  })
}

/**
 * Check whether a path should be excluded from local search results.
 * Returns `true` for secret paths, generated/dependency dirs, and blocked
 * protected paths. Always resolves (never rejects).
 */
export function searchShouldExclude(path: string, workspaceRoot: string): Promise<boolean> {
  return invoke<boolean>('search_should_exclude', { path, workspaceRoot })
}

// ===========================================================================
// Extension Permission Foundation (M14 Wave 6)
//
// Typed wrappers over the engine-backed `permissions_*` commands. The
// permission registry and approval history are stored locally in
// `.gwenland/extensions/`. No extension runtime is implemented in M14.
// ===========================================================================

/** Known extension permission kinds (mirrors `gwenland_engine::permissions::Permission`). */
export type ExtensionPermission =
  | 'read_workspace'
  | 'write_file'
  | 'delete_file'
  | 'run_terminal'
  | 'access_git'
  | 'access_env'
  | 'access_database'
  | 'unknown'

/** Default policy verdict for a permission (mirrors `PermissionDefault`). */
export type PermissionDefault = 'allowed' | 'ask' | 'blocked'

/**
 * Resolved permission decision for one extension+permission pair
 * (mirrors `PermissionDecision`).
 */
export interface PermissionDecision {
  extension_id: string
  permission: string
  verdict: PermissionDefault
  reason: string
}

/**
 * Load the effective permission state for an extension. Returns the resolved
 * verdict (from the per-workspace registry + default matrix) for every known
 * permission kind. Always resolves — falls back to the default matrix when
 * the registry file is absent or malformed.
 */
export function permissionsLoadState(
  workspaceRoot: string,
  extensionId: string
): Promise<PermissionDecision[]> {
  return invoke<PermissionDecision[]>('permissions_load_state', {
    workspaceRoot,
    extensionId,
  })
}

/**
 * Record an extension permission approval or denial in the workspace approval
 * history (`.gwenland/extensions/approvals.jsonl`). The `targetSummary` is
 * bounded and redacted by the engine before writing — never include secrets.
 */
export function permissionsRecordApproval(
  workspaceRoot: string,
  extensionId: string,
  permission: string,
  approved: boolean,
  targetSummary: string
): Promise<void> {
  return invoke<void>('permissions_record_approval', {
    workspaceRoot,
    extensionId,
    permission,
    approved,
    targetSummary,
  })
}
