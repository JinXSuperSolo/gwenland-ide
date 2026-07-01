import type { ModelEntry, TokenUsage } from '../tauri/commands'

/**
 * Token-usage/cost helpers (GWEN-457). Pure, dependency-free, and testable —
 * mirrors `lib/ai/reasoning.ts`'s pattern of taking an already-loaded catalog
 * rather than fetching it itself.
 */

/** USD cost of one request given its token usage and the model's catalog pricing. */
export function computeCostUsd(usage: TokenUsage, entry: ModelEntry): number {
  return (
    (usage.input_tokens / 1_000_000) * entry.pricing.input_per_m +
    (usage.output_tokens / 1_000_000) * entry.pricing.output_per_m
  )
}

/** Compact token count, e.g. `1234` -> `"1.2K"`, `340` -> `"340"`. */
export function formatTokenCount(n: number): string {
  if (n < 1000) return String(n)
  return `${(n / 1000).toFixed(1)}K`
}

/** Format a USD amount for the per-message usage line, e.g. `$0.008`, `<$0.001`. */
export function formatCostUsd(cost: number): string {
  if (cost === 0) return '$0'
  if (cost < 0.001) return '<$0.001'
  return `$${cost.toFixed(3)}`
}

/**
 * One-line summary for a message's usage footer, e.g.
 * `"1.2K in · 340 out · $0.008"`. Returns `null` when pricing for the turn's
 * model isn't in the catalog (e.g. a custom/generic provider model), so the
 * caller can omit the cost portion rather than show a wrong number.
 */
export function usageSummary(
  usage: TokenUsage,
  provider: string | undefined,
  model: string | undefined,
  catalog: ModelEntry[]
): string {
  const tokensPart = `${formatTokenCount(usage.input_tokens)} in · ${formatTokenCount(usage.output_tokens)} out`
  const entry = catalog.find((m) => m.id === model)
  if (!entry || !provider) return tokensPart
  return `${tokensPart} · ${formatCostUsd(computeCostUsd(usage, entry))}`
}
