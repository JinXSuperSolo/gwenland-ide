import type { ContextAction, ContextMenuContext } from './contextTypes'

/**
 * The single source of truth for every context-menu action (Requirement 1).
 * Feature areas register their actions here at app init (fileActions,
 * editorActions, …) and panels never branch on scope themselves — they just
 * call `openContextMenu(event, ctx)` and the registry returns the applicable,
 * sorted, grouped actions.
 *
 * Ordering: actions sort by their group's *minimum* order first, then by their
 * own `order`. Assigning each group a contiguous order band therefore lays the
 * groups out in the intended sequence without a separate group-order table.
 */
export class ContextActionRegistry {
  private actions = new Map<string, ContextAction>()

  /** Register (or replace, by id) a single action. */
  register(action: ContextAction): void {
    this.actions.set(action.id, action)
  }

  /** Register many actions at once. */
  registerAll(actions: ContextAction[]): void {
    for (const action of actions) this.register(action)
  }

  /** Remove an action by id (idempotent). Used mainly by tests. */
  unregister(id: string): void {
    this.actions.delete(id)
  }

  /** Every registered action, unfiltered. Used mainly by tests. */
  all(): ContextAction[] {
    return [...this.actions.values()]
  }

  /**
   * Actions that apply to `ctx` (passing `when`), sorted by group then order.
   * Disabled actions are still returned — disabling is a render concern, not a
   * filtering one (Requirement 6.2 / 8.1).
   */
  getActions(ctx: ContextMenuContext): ContextAction[] {
    const filtered = [...this.actions.values()].filter((a) => safeWhen(a, ctx))

    // Each group sorts by the lowest order among its members, so groups keep
    // their intended sequence even if a group's actions were registered apart.
    const groupMin = new Map<string, number>()
    for (const a of filtered) {
      const cur = groupMin.get(a.group)
      if (cur === undefined || a.order < cur) groupMin.set(a.group, a.order)
    }

    return filtered.sort((a, b) => {
      const ga = groupMin.get(a.group) ?? 0
      const gb = groupMin.get(b.group) ?? 0
      if (ga !== gb) return ga - gb
      if (a.group !== b.group) return a.group < b.group ? -1 : 1
      return a.order - b.order
    })
  }

  /**
   * Applicable actions split into contiguous groups, in render order. The portal
   * renders a separator between consecutive groups.
   */
  getGrouped(ctx: ContextMenuContext): { group: string; actions: ContextAction[] }[] {
    const groups: { group: string; actions: ContextAction[] }[] = []
    for (const action of this.getActions(ctx)) {
      let bucket = groups.find((g) => g.group === action.group)
      if (!bucket) {
        bucket = { group: action.group, actions: [] }
        groups.push(bucket)
      }
      bucket.actions.push(action)
    }
    return groups
  }
}

/** A `when` that throws must not take the whole menu down — treat it as hidden. */
function safeWhen(action: ContextAction, ctx: ContextMenuContext): boolean {
  try {
    return action.when(ctx)
  } catch {
    return false
  }
}

/**
 * Whether an action is enabled for `ctx`. A missing `enabled` defaults to true;
 * a throwing predicate degrades to disabled rather than crashing the menu
 * (Requirement 8.1).
 */
export function isActionEnabled(action: ContextAction, ctx: ContextMenuContext): boolean {
  if (!action.enabled) return true
  try {
    return action.enabled(ctx)
  } catch {
    return false
  }
}

/** App-wide singleton used by both the menu shell and every action module. */
export const registry = new ContextActionRegistry()
