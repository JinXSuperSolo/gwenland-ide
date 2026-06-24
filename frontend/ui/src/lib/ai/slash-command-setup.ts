import { get } from 'svelte/store'

import * as cmd from '../tauri/commands'
import { aiChat, type ChatMessage } from '../stores/ai-chat'
import { createConversation, projectRoot } from './ai-chat-setup'
import { parseHistoryCount, type SlashCommandId } from '../stores/slash-commands'
import { loadPersona, savePersona, personaState } from '../stores/workspace-persona'

/**
 * Slash-command behaviour (GWEN-333). The composer parses + dispatches by id;
 * this module performs the side effects (mutating the chat store, calling the
 * model, writing workspace files). Kept separate from the Tauri-free registry in
 * `../stores/slash-commands.ts`, matching the `ai-chat` store/setup split.
 *
 * Every handler is best-effort: failures surface in the AI panel's `lastError`
 * banner rather than throwing, so a bad command never breaks the composer.
 */

/** What a command dispatch asks the composer to do next. */
export interface SlashResult {
  /** Replace the composer text with this (e.g. clear it after running). */
  setInput?: string
  /** Open the inline model picker. */
  openModelPicker?: boolean
  /** Open the inline mode/tier picker. */
  openModePicker?: boolean
  /** Open the inline persona picker (name + tone). */
  openPersonaPicker?: boolean
  /** Open the inline system-prompt editor. */
  openSystemEditor?: boolean
}

function setError(message: string): void {
  aiChat.update((s) => ({ ...s, lastError: { kind: 'storage_error', data: message } }))
}

/** Append a display-only assistant message (not persisted; JSONL only grows on
 *  a real completion's `ai://done`). Used by /compact and /get-history. */
function pushAssistantNote(content: string): void {
  const msg: ChatMessage = {
    id: crypto.randomUUID(),
    role: 'assistant',
    content,
    streaming: false,
    timestamp: new Date().toISOString(),
  }
  aiChat.update((s) => ({ ...s, messages: [...s.messages, msg] }))
}

// --- /clear ----------------------------------------------------------------

/** Drop the in-memory messages for the active conversation (UI only). */
function clearMessages(): SlashResult {
  aiChat.update((s) => ({ ...s, messages: [], lastError: null }))
  return { setInput: '' }
}

// --- /compact --------------------------------------------------------------

const COMPACT_PROMPT =
  'Summarize this conversation history concisely. Preserve key decisions, code ' +
  'snippets referenced, and context needed to continue.'

/**
 * Ask the model to summarize the visible history, then replace the messages
 * with a single summary block. No-op (with a note) when there's nothing to
 * compact or no model is configured.
 */
async function compactHistory(): Promise<SlashResult> {
  const state = get(aiChat)
  if (state.messages.length === 0) return { setInput: '' }
  if (!state.activeModel) {
    setError('Select a model before compacting.')
    return { setInput: '' }
  }
  const transcript = state.messages
    .map((m) => `${m.role === 'user' ? 'User' : 'Assistant'}: ${m.content}`)
    .join('\n\n')
  try {
    const summary = await cmd.aiComplete(
      `${COMPACT_PROMPT}\n\n---\n${transcript}`,
      state.activeProvider,
      state.activeModel
    )
    const block: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'assistant',
      content: `**Compacted summary**\n\n${summary.trim()}`,
      streaming: false,
      timestamp: new Date().toISOString(),
    }
    aiChat.update((s) => ({ ...s, messages: [block], lastError: null }))
  } catch (e) {
    setError(`Could not compact: ${String(e)}`)
  }
  return { setInput: '' }
}

// --- /get-history ----------------------------------------------------------

/**
 * Load the last `n` messages from the active conversation's JSONL and inject
 * them as a read-only context block. Defaults to 5. Falls back to the in-memory
 * messages when nothing is persisted yet.
 */
async function getHistory(rest: string): Promise<SlashResult> {
  const n = parseHistoryCount(rest)
  const state = get(aiChat)
  const convId = state.activeConversationId
  let lines: string[] = []
  if (convId) {
    try {
      const turns = await cmd.conversationLoad(convId)
      const flat = turns.flatMap((t) => t.messages)
      lines = flat.slice(-n).map((m) => `**${m.role}:** ${m.content}`)
    } catch (e) {
      setError(`Could not load history: ${String(e)}`)
      return { setInput: '' }
    }
  }
  if (lines.length === 0) {
    pushAssistantNote('_No prior history to load._')
    return { setInput: '' }
  }
  pushAssistantNote(`**Last ${lines.length} message(s):**\n\n${lines.join('\n\n')}`)
  return { setInput: '' }
}

// --- /add-ctx-folder -------------------------------------------------------

/** Local date stamp (YYYY-MM-DD) for the template footers. */
function today(): string {
  return new Date().toISOString().slice(0, 10)
}

function contextTemplate(folderName: string): string {
  return `# Context: ${folderName}

## Purpose

[What this folder contains / does]

## Key Files

[List of important files and their role]

## AI Instructions

[Custom instructions for AI when working in this folder]

---
Last updated: ${today()}
`
}

/**
 * Prompt for a folder, then create `CONTEXT.md` inside it from the template.
 * Uses the native folder picker so the user explicitly chooses the target.
 */
async function addContextFolder(): Promise<SlashResult> {
  const root = projectRoot()
  if (!root) {
    setError('Open a project folder first.')
    return { setInput: '' }
  }
  let folder: string
  try {
    folder = await cmd.openFolderDialog()
  } catch {
    // User cancelled the dialog — silent no-op.
    return { setInput: '' }
  }
  const folderName = folder.split(/[\\/]/).filter(Boolean).pop() || folder
  const filePath = `${folder}/CONTEXT.md`
  try {
    await cmd.writeFile(filePath, contextTemplate(folderName))
    pushAssistantNote(`Created \`CONTEXT.md\` in **${folderName}**.`)
  } catch (e) {
    setError(`Could not write CONTEXT.md: ${String(e)}`)
  }
  return { setInput: '' }
}

// --- /setup ----------------------------------------------------------------

/** Best-effort project name/lang/framework detection from manifest files. */
interface ProjectInfo {
  name: string
  language: string
  framework: string
}

async function detectProject(root: string, entries: cmd.DirEntry[]): Promise<ProjectInfo> {
  const has = (name: string) => entries.some((e) => !e.is_dir && e.name === name)
  const info: ProjectInfo = { name: '[unknown]', language: '[unknown]', framework: '[none detected]' }

  // package.json → JS/TS project; sniff a framework from its deps.
  if (has('package.json')) {
    info.language = has('tsconfig.json') ? 'TypeScript' : 'JavaScript'
    try {
      const pkgRaw = await cmd.readFile(`${root}/package.json`)
      const pkg = JSON.parse(pkgRaw) as {
        name?: string
        dependencies?: Record<string, string>
        devDependencies?: Record<string, string>
      }
      if (pkg.name) info.name = pkg.name
      const deps = { ...pkg.dependencies, ...pkg.devDependencies }
      if (deps.svelte) info.framework = 'Svelte'
      else if (deps.react) info.framework = 'React'
      else if (deps.vue) info.framework = 'Vue'
      else if (deps['@angular/core']) info.framework = 'Angular'
      else if (deps.next) info.framework = 'Next.js'
    } catch {
      /* leave defaults */
    }
  }

  // Cargo.toml → Rust (Tauri when src-tauri/ is present).
  if (has('Cargo.toml')) {
    info.language = info.language === '[unknown]' ? 'Rust' : `${info.language} + Rust`
    try {
      const cargo = await cmd.readFile(`${root}/Cargo.toml`)
      const m = cargo.match(/^\s*name\s*=\s*"([^"]+)"/m)
      if (m && info.name === '[unknown]') info.name = m[1]
    } catch {
      /* leave defaults */
    }
  }
  if (entries.some((e) => e.is_dir && e.name === 'src-tauri')) info.framework = 'Tauri'

  // pyproject.toml / requirements.txt → Python.
  if (has('pyproject.toml') || has('requirements.txt')) {
    info.language = info.language === '[unknown]' ? 'Python' : info.language
  }

  return info
}

function setupTemplate(
  root: string,
  info: ProjectInfo,
  keyFiles: string[]
): string {
  const files = keyFiles.length ? keyFiles.join(', ') : '[none detected]'
  // Canonical GwenLand.md format (GWEN-334). Persona/System Prompt/Custom
  // Instructions start as placeholders; `parseGwenLandMd` treats `[...]` lines as
  // empty so the engine default applies until the user fills them in.
  return `# GwenLand Workspace Config

## Workspace
- Root: ${root}
- Language: ${info.language}
- Framework: ${info.framework}
- Key files: ${files}

## AI Persona
- Name: GwenLand AI
- Tone: casual
- Language: [unset]
- Focus: [unset]

## System Prompt
[default empty — user fills in]

## Custom Instructions
[default empty — user fills in]

## Workspace Context
- What this project does: [describe]
- Last updated: ${today()}
`
}

/** Config/entry files worth surfacing if present at the root. */
const NOTABLE_FILES = [
  'package.json',
  'Cargo.toml',
  'pyproject.toml',
  'tsconfig.json',
  'vite.config.ts',
  'vite.config.js',
  'tauri.conf.json',
  'README.md',
  'main.ts',
  'main.rs',
  'index.ts',
  'index.js',
]

/**
 * Scan the open workspace root and generate `.gwenland/GwenLand.md`. Reuses the
 * existing `list_directory` command for the scan and the workspace-scoped
 * `create_dir` + `write_file` for output.
 */
async function setupWorkspace(): Promise<SlashResult> {
  const root = projectRoot()
  if (!root) {
    setError('Open a project folder first.')
    return { setInput: '' }
  }
  try {
    const entries = await cmd.listDirectory(root)
    const keyFiles = entries
      .filter((e) => !e.is_dir && NOTABLE_FILES.includes(e.name))
      .map((e) => e.name)
    const info = await detectProject(root, entries)

    // Ensure `.gwenland/` exists (idempotent: create_dir rejects if present, so
    // ignore that specific failure), then write the doc.
    try {
      await cmd.createDir(`${root}/.gwenland`, root)
    } catch {
      /* already exists — fine */
    }
    const content = setupTemplate(root, info, keyFiles)
    await cmd.writeFile(`${root}/.gwenland/GwenLand.md`, content)
    // Reflect the new config in the persona store (header + system prompt).
    await loadPersona()
    pushAssistantNote('Generated `.gwenland/GwenLand.md` from the workspace scan.')
  } catch (e) {
    setError(`Could not run setup: ${String(e)}`)
  }
  return { setInput: '' }
}

// --- /reset-system ---------------------------------------------------------

/**
 * Clear the workspace's custom system prompt so the default GwenLand prompt
 * applies again, persisting the change to GwenLand.md. Persona name/tone and
 * custom instructions are left intact.
 */
async function resetSystemPrompt(): Promise<SlashResult> {
  if (!projectRoot()) {
    setError('Open a project folder first.')
    return { setInput: '' }
  }
  try {
    const config = { ...personaState(), systemPrompt: '' }
    await savePersona(config)
    pushAssistantNote('System prompt reset to the GwenLand default.')
  } catch (e) {
    setError(`Could not reset system prompt: ${String(e)}`)
  }
  return { setInput: '' }
}

// --- Dispatch --------------------------------------------------------------

/**
 * Run a slash command by id. `rest` is the raw argument text after the command
 * word (used by `/get-history`). Returns a `SlashResult` telling the composer
 * what to do next (clear input, open a picker). Unknown ids are a no-op.
 */
export async function runSlashCommand(
  id: SlashCommandId,
  rest = ''
): Promise<SlashResult> {
  switch (id) {
    case 'clear':
      return clearMessages()
    case 'new':
      await createConversation()
      return { setInput: '' }
    case 'compact':
      return compactHistory()
    case 'get-history':
      return getHistory(rest)
    case 'model':
      return { setInput: '', openModelPicker: true }
    case 'mode':
      return { setInput: '', openModePicker: true }
    case 'persona':
      return { setInput: '', openPersonaPicker: true }
    case 'system':
      return { setInput: '', openSystemEditor: true }
    case 'reset-system':
      return resetSystemPrompt()
    case 'add-ctx-folder':
      return addContextFolder()
    case 'setup':
      return setupWorkspace()
    default:
      return {}
  }
}
