/**
 * Shared types for the M9 Context Menu System.
 *
 * The whole system is registry-driven: every right-clickable surface builds a
 * `ContextMenuContext` describing *what* was clicked (never *which menu*), and a
 * central registry decides which `ContextAction`s apply. No panel ships its own
 * menu component — see [`actionRegistry`](./actionRegistry.ts).
 */

/** Every surface that can open a context menu. New surfaces add a variant here. */
export type ContextMenuScope =
  | 'file_tree'
  | 'editor'
  | 'editor_tab'
  | 'workspace_empty'
  | 'terminal'
  | 'problems'
  | 'search'
  | 'git'
  | 'ai_chat'
  | 'panel_header'
  /** Right-click on a text field anywhere (Cut/Copy/Paste/Select All). */
  | 'input'
  /** Window-level fallback so the IDE menu (never the native one) always shows. */
  | 'global'

/**
 * The data a surface hands to `openContextMenu`. Every field beyond `scope` is
 * optional: actions MUST NOT assume a field is present unless they gate on it in
 * `when`/`enabled` (Requirement 8.2). Add fields as new surfaces need them.
 */
export interface ContextMenuContext {
  scope: ContextMenuScope
  /** Absolute path of the open project folder, when one is open. */
  workspaceRoot?: string
  /** The primary path the menu acts on (file/folder/tab path). */
  path?: string
  /** Multi-select paths (deferred; reserved so actions can opt in later). */
  paths?: string[]
  /** True when `path` is a directory (file-tree nodes). */
  isDirectory?: boolean
  /** Editor language id, used to gate LSP actions. */
  languageId?: string
  /** Current editor selection text, used to gate selection-aware actions. */
  selectionText?: string
  /** Current terminal selection text. */
  terminalSelection?: string
  /** Identifier of the clicked problem/diagnostic row. */
  problemId?: string
  /** Human-readable text of the clicked problem/diagnostic (Copy Message). */
  message?: string
  /** Git status code for the clicked change (e.g. "M", "??"). */
  gitStatus?: string
  /** Identifier of the panel the menu was opened from. */
  panelId?: string
  /** Id of the terminal session the menu was opened from. */
  terminalId?: string
  /** Id of the editor tab the menu was opened from (editor_tab scope). */
  tabId?: string
  /** Id of the editor group the menu was opened from. */
  groupId?: string
}

/**
 * A single registerable action. `when` decides visibility (filtered out of the
 * menu entirely); `enabled` decides the disabled-but-shown state — capabilities
 * that are temporarily unavailable (LSP/git) disable, they do not vanish
 * (Requirement 6.2 / 8.1).
 */
export interface ContextAction {
  id: string
  label: string
  /** Icon name from the `Icon.svelte` registry (rendered when present). */
  icon?: string
  /** Section key; actions in different groups are split by a separator. */
  group: string
  /** Sort order within (and, via the group's min, across) groups. */
  order: number
  /** Display-only shortcut hint, e.g. "Ctrl+C". Used when no `commandId` maps. */
  shortcut?: string
  /** Command id to pull the live display shortcut from (M2 registry, Task 5.2).
   *  Takes precedence over `shortcut` so menus match the palette/menu-bar hint. */
  commandId?: string
  /** Visibility predicate — return false to omit the action entirely. */
  when: (ctx: ContextMenuContext) => boolean
  /** Enablement predicate — return false to render disabled. Defaults to true. */
  enabled?: (ctx: ContextMenuContext) => boolean
  /** The effect. May be async; the menu closes before/while it runs. */
  run: (ctx: ContextMenuContext) => Promise<void> | void
}
