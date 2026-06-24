/**
 * Problems / Search / Git panel context actions (Requirement 7.2-7.6).
 *
 * These panels aren't built yet, so per Wave 4 these actions are *registered now*
 * and will light up when the panels start calling `openContextMenu` with the
 * matching scope (Task 4.4). Path-based operations (open, reveal, copy path) are
 * fully implemented against existing primitives; operations needing a git /
 * search / diagnostics backend that doesn't exist yet degrade to a non-blocking
 * notice. Dangerous actions (Discard Changes, Replace All) confirm first
 * (Requirement 7.5) — the confirmation is wired regardless of backend.
 *
 * All three panels share this one registry — no per-panel menu component
 * (Requirement 7.6).
 */
import { registry } from '../context-menu/actionRegistry'
import type { ContextAction } from '../context-menu/contextTypes'
import { openFile } from '../stores/tabs'
import { expandPanel } from '../stores/panels'
import { requestTreeReveal } from '../stores/file-tree'

function basename(p: string): string {
  return p.split(/[\\/]/).filter(Boolean).pop() || p
}

/** Make a file visible in the Explorer (expand panel + reveal ancestors). */
function revealInTree(path: string): void {
  expandPanel('fileTree')
  requestTreeReveal(path)
}

/** Graceful placeholder for an op whose panel/backend isn't wired yet. */
function pending(name: string): void {
  console.info(`[GwenLand] "${name}" will work once its panel/backend lands.`)
}

const copyText = (text: string) => void navigator.clipboard.writeText(text).catch(() => {})

// --- Problems panel --------------------------------------------------------
const problemsActions: ContextAction[] = [
  {
    id: 'problems.open',
    label: 'Open Problem',
    icon: 'warning-circle',
    group: 'open',
    order: 10,
    when: (ctx) => ctx.scope === 'problems',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) void openFile(ctx.path)
    },
  },
  {
    id: 'problems.copyMessage',
    label: 'Copy Message',
    icon: 'copy',
    group: 'clipboard',
    order: 20,
    when: (ctx) => ctx.scope === 'problems',
    enabled: (ctx) => !!ctx.message,
    run: (ctx) => {
      if (ctx.message) copyText(ctx.message)
    },
  },
  {
    id: 'problems.copyDiagnostic',
    label: 'Copy Diagnostic',
    icon: 'copy',
    group: 'clipboard',
    order: 30,
    when: (ctx) => ctx.scope === 'problems',
    enabled: (ctx) => !!ctx.message,
    run: (ctx) => {
      if (!ctx.message) return
      copyText(ctx.path ? `${ctx.path}: ${ctx.message}` : ctx.message)
    },
  },
  {
    id: 'problems.revealFile',
    label: 'Reveal File',
    icon: 'eye',
    group: 'navigate',
    order: 40,
    when: (ctx) => ctx.scope === 'problems',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) revealInTree(ctx.path)
    },
  },
  {
    id: 'problems.clearFilter',
    label: 'Clear Filter',
    icon: 'refresh',
    group: 'view',
    order: 50,
    when: (ctx) => ctx.scope === 'problems',
    run: () => pending('Clear Filter'),
  },
]

// --- Search panel ----------------------------------------------------------
const searchActions: ContextAction[] = [
  {
    id: 'search.open',
    label: 'Open Result',
    icon: 'page',
    group: 'open',
    order: 10,
    when: (ctx) => ctx.scope === 'search',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) void openFile(ctx.path)
    },
  },
  {
    id: 'search.reveal',
    label: 'Reveal in File Tree',
    icon: 'eye',
    group: 'navigate',
    order: 20,
    when: (ctx) => ctx.scope === 'search',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) revealInTree(ctx.path)
    },
  },
  {
    id: 'search.copyPath',
    label: 'Copy Path',
    icon: 'copy',
    group: 'clipboard',
    order: 30,
    when: (ctx) => ctx.scope === 'search',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) copyText(ctx.path)
    },
  },
  {
    id: 'search.replaceThis',
    label: 'Replace This',
    icon: 'text',
    group: 'edit',
    order: 40,
    when: (ctx) => ctx.scope === 'search',
    run: () => pending('Replace This'),
  },
  {
    id: 'search.replaceAll',
    label: 'Replace All in File',
    icon: 'text',
    group: 'edit',
    order: 50,
    when: (ctx) => ctx.scope === 'search',
    // Dangerous — confirm first (Requirement 7.5).
    run: () => {
      if (!confirm('Replace all matches in this file? This cannot be undone.')) return
      pending('Replace All in File')
    },
  },
]

// --- Git panel -------------------------------------------------------------
const gitActions: ContextAction[] = [
  {
    id: 'git.openFile',
    label: 'Open File',
    icon: 'page',
    group: 'open',
    order: 10,
    when: (ctx) => ctx.scope === 'git',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) void openFile(ctx.path)
    },
  },
  {
    id: 'git.viewDiff',
    label: 'View Diff',
    icon: 'code',
    group: 'open',
    order: 20,
    when: (ctx) => ctx.scope === 'git',
    enabled: (ctx) => !!ctx.path,
    run: () => pending('View Diff'),
  },
  {
    id: 'git.stage',
    label: 'Stage',
    icon: 'plus',
    group: 'edit',
    order: 30,
    when: (ctx) => ctx.scope === 'git',
    enabled: (ctx) => !!ctx.path,
    run: () => pending('Stage'),
  },
  {
    id: 'git.unstage',
    label: 'Unstage',
    icon: 'reply',
    group: 'edit',
    order: 40,
    when: (ctx) => ctx.scope === 'git',
    enabled: (ctx) => !!ctx.path,
    run: () => pending('Unstage'),
  },
  {
    id: 'git.discard',
    label: 'Discard Changes',
    icon: 'bin',
    group: 'edit',
    order: 50,
    when: (ctx) => ctx.scope === 'git',
    enabled: (ctx) => !!ctx.path,
    // Dangerous — confirm first (Requirement 7.5).
    run: (ctx) => {
      const name = ctx.path ? basename(ctx.path) : 'this file'
      if (!confirm(`Discard changes to "${name}"? This cannot be undone.`)) return
      pending('Discard Changes')
    },
  },
  {
    id: 'git.copyPath',
    label: 'Copy Path',
    icon: 'copy',
    group: 'clipboard',
    order: 60,
    when: (ctx) => ctx.scope === 'git',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) copyText(ctx.path)
    },
  },
  {
    id: 'git.reveal',
    label: 'Reveal in File Tree',
    icon: 'eye',
    group: 'navigate',
    order: 70,
    when: (ctx) => ctx.scope === 'git',
    enabled: (ctx) => !!ctx.path,
    run: (ctx) => {
      if (ctx.path) revealInTree(ctx.path)
    },
  },
]

/** Register the problems/search/git action sets into the shared registry. */
export function registerPanelActions(): void {
  registry.registerAll(problemsActions)
  registry.registerAll(searchActions)
  registry.registerAll(gitActions)
}
