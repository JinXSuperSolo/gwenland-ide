import type { ModelEntry } from '../tauri/commands'

/**
 * Reasoning-level helpers (Requirement 6, GWEN-458). Catalog-driven: the
 * levels offered for a model come from that model's `ModelEntry.reasoning`
 * entry (see `ai/model_catalog.rs`), not a fixed cross-provider list — e.g.
 * Anthropic Opus offers low/medium/high/xhigh/max, Grok offers
 * none/low/medium/high, and plenty of models offer none at all.
 *
 * Pure, dependency-free, and testable: callers pass in the already-loaded
 * catalog (`ComposerModelMenu`/`ReasoningMenu` fetch it once via
 * `aiModelCatalog()`), no I/O happens here.
 */

/** Whether the active provider/model should expose a reasoning control at all. */
export function isThinkingCapable(catalog: ModelEntry[], model: string): boolean {
  return catalog.find((m) => m.id === model)?.reasoning.supported ?? false
}

/** The reasoning levels offered by `model`, in catalog order (empty if unsupported). */
export function reasoningLevelsFor(catalog: ModelEntry[], model: string): string[] {
  return catalog.find((m) => m.id === model)?.reasoning.levels ?? []
}

/** The model's default reasoning level, or `null` if it has none/is unsupported. */
export function defaultReasoningLevelFor(catalog: ModelEntry[], model: string): string | null {
  return catalog.find((m) => m.id === model)?.reasoning.default ?? null
}

/** Human-friendly label for a level id, e.g. `xhigh` -> `Extra High`. */
export function reasoningLevelLabel(level: string): string {
  const known: Record<string, string> = {
    none: 'None',
    off: 'Off',
    low: 'Low',
    medium: 'Medium',
    high: 'High',
    xhigh: 'Extra High',
    max: 'Max',
    on: 'On',
  }
  return known[level] ?? level.charAt(0).toUpperCase() + level.slice(1)
}
