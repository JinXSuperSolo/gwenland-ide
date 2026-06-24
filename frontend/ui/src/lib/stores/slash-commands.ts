/**
 * Slash command registry + autocomplete state (GWEN-333).
 *
 * Typing `/` in the AI composer opens a filterable dropdown of commands. This
 * module owns only the *declarative* registry (name, description, optional arg
 * hint) and the pure parse/filter helpers — it is Tauri-free and side-effect
 * free, mirroring how `ai-chat.ts` stays decoupled from `ai-chat-setup.ts`.
 *
 * The actual command behaviour (clearing history, calling the model to compact,
 * writing CONTEXT.md, …) lives in `../ai/slash-command-setup.ts`, which the
 * composer dispatches to by command id.
 */

/** Stable identifier for each command (matches the leading token, sans `/`). */
export type SlashCommandId =
  | 'clear'
  | 'new'
  | 'compact'
  | 'get-history'
  | 'model'
  | 'mode'
  | 'persona'
  | 'system'
  | 'reset-system'
  | 'add-ctx-folder'
  | 'setup'

export interface SlashCommand {
  id: SlashCommandId
  /** The literal typed token, e.g. `/get-history`. */
  name: string
  /** One-line description shown in the dropdown row. */
  description: string
  /** Inline argument hint shown after the name, e.g. `[n=5]`. Omitted = none. */
  argHint?: string
}

/**
 * The available commands, grouped conceptually (conversation / model+mode /
 * persona / workspace) but rendered as one flat filtered list. Order here is the
 * default (unfiltered) display order.
 */
export const SLASH_COMMANDS: SlashCommand[] = [
  // Conversation
  { id: 'clear', name: '/clear', description: 'Clear messages in this conversation' },
  { id: 'new', name: '/new', description: 'Start a fresh conversation' },
  { id: 'compact', name: '/compact', description: 'Summarize history into a single block' },
  {
    id: 'get-history',
    name: '/get-history',
    description: 'Inject the last messages as context',
    argHint: '[n=5]',
  },
  // Model & mode
  { id: 'model', name: '/model', description: 'Pick the provider / model' },
  { id: 'mode', name: '/mode', description: 'Switch autonomy mode' },
  // Persona & system prompt (GWEN-334)
  { id: 'persona', name: '/persona', description: 'Set AI name & tone preset' },
  { id: 'system', name: '/system', description: 'Edit the workspace system prompt' },
  { id: 'reset-system', name: '/reset-system', description: 'Restore the default system prompt' },
  // Workspace
  {
    id: 'add-ctx-folder',
    name: '/add-ctx-folder',
    description: 'Create CONTEXT.md in a folder',
  },
  { id: 'setup', name: '/setup', description: 'Generate .gwenland/GwenLand.md' },
]

/** Lookup by id (used by the dispatcher). */
export function commandById(id: SlashCommandId): SlashCommand | undefined {
  return SLASH_COMMANDS.find((c) => c.id === id)
}

/**
 * A parsed slash query from the composer text. Only the FIRST line is
 * considered, and only when it starts with `/`. `token` is the command word
 * (without the slash), `rest` is everything after the first space (the args).
 */
export interface SlashQuery {
  /** The command word without the leading slash, lowercased, e.g. `get-history`. */
  token: string
  /** Raw argument text after the first whitespace (may be empty). */
  rest: string
  /** True once the user typed a space — args are being entered, not the name. */
  hasArgs: boolean
}

/**
 * Parse composer text into a slash query, or null when it isn't a slash command
 * invocation. We only trigger on text whose first non-empty content begins with
 * `/` and contains no newline before the command word, so a `/` inside a normal
 * multi-line prompt doesn't pop the menu.
 */
export function parseSlashQuery(text: string): SlashQuery | null {
  if (!text.startsWith('/')) return null
  // Restrict to the first line: a `/` that starts a later line is just prose.
  const firstLine = text.split('\n', 1)[0]
  const body = firstLine.slice(1) // drop the leading slash
  const spaceIdx = body.search(/\s/)
  if (spaceIdx === -1) {
    return { token: body.toLowerCase(), rest: '', hasArgs: false }
  }
  return {
    token: body.slice(0, spaceIdx).toLowerCase(),
    rest: body.slice(spaceIdx + 1),
    hasArgs: true,
  }
}

/**
 * Commands matching a query token, by case-insensitive prefix then substring.
 * An empty token returns all commands (the bare-`/` state). Exact full-name
 * matches still appear so the row stays highlightable.
 */
export function filterCommands(token: string): SlashCommand[] {
  const t = token.toLowerCase()
  if (!t) return SLASH_COMMANDS
  const prefix = SLASH_COMMANDS.filter((c) => c.id.startsWith(t))
  const substr = SLASH_COMMANDS.filter((c) => !c.id.startsWith(t) && c.id.includes(t))
  return [...prefix, ...substr]
}

/** Exact command for a fully-typed token (`get-history` → the command), else null. */
export function exactCommand(token: string): SlashCommand | null {
  return SLASH_COMMANDS.find((c) => c.id === token.toLowerCase()) ?? null
}

/**
 * Parse the `[n]` argument for `/get-history`. Defaults to 5, clamps to 1..50,
 * ignores trailing junk. Non-numeric input falls back to the default.
 */
export function parseHistoryCount(rest: string): number {
  const n = parseInt(rest.trim(), 10)
  if (!Number.isFinite(n) || n <= 0) return 5
  return Math.min(n, 50)
}
