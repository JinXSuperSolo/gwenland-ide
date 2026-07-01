import { describe, expect, it } from 'vitest'
import { computeCostUsd, formatCostUsd, formatTokenCount, usageSummary } from './usage'
import type { ModelEntry } from '../tauri/commands'

const opus: ModelEntry = {
  id: 'claude-opus-4-8',
  name: 'Claude Opus 4.8',
  provider: 'anthropic',
  tier: 'flagship',
  context_window: 200_000,
  pricing: { input_per_m: 5.0, output_per_m: 25.0 },
  reasoning: { supported: true, param_name: 'output_config.effort', levels: ['low', 'high'], default: 'high', mode: 'effort' },
}

describe('computeCostUsd', () => {
  it('combines input and output token cost at per-1M rates', () => {
    const cost = computeCostUsd({ input_tokens: 1_000_000, output_tokens: 1_000_000 }, opus)
    expect(cost).toBeCloseTo(30.0, 6)
  })

  it('scales down for small token counts', () => {
    const cost = computeCostUsd({ input_tokens: 1000, output_tokens: 500 }, opus)
    // 1000/1e6 * 5 + 500/1e6 * 25 = 0.005 + 0.0125 = 0.0175
    expect(cost).toBeCloseTo(0.0175, 6)
  })

  it('is zero for zero usage', () => {
    expect(computeCostUsd({ input_tokens: 0, output_tokens: 0 }, opus)).toBe(0)
  })
})

describe('formatTokenCount', () => {
  it('shows small counts verbatim', () => {
    expect(formatTokenCount(340)).toBe('340')
    expect(formatTokenCount(0)).toBe('0')
    expect(formatTokenCount(999)).toBe('999')
  })

  it('abbreviates thousands with one decimal', () => {
    expect(formatTokenCount(1200)).toBe('1.2K')
    expect(formatTokenCount(1000)).toBe('1.0K')
    expect(formatTokenCount(15750)).toBe('15.8K')
  })
})

describe('formatCostUsd', () => {
  it('shows $0 for exactly zero', () => {
    expect(formatCostUsd(0)).toBe('$0')
  })

  it('floors tiny nonzero costs to a <$0.001 label instead of rounding to $0.000', () => {
    expect(formatCostUsd(0.0000001)).toBe('<$0.001')
  })

  it('shows 3 decimal places for normal costs', () => {
    expect(formatCostUsd(0.0175)).toBe('$0.018')
    expect(formatCostUsd(1.5)).toBe('$1.500')
  })
})

describe('usageSummary', () => {
  const usage = { input_tokens: 1200, output_tokens: 340 }

  it('includes cost when the model is in the catalog', () => {
    const line = usageSummary(usage, 'anthropic', 'claude-opus-4-8', [opus])
    expect(line).toBe('1.2K in · 340 out · $0.014')
  })

  it('omits cost when the model is not in the catalog (e.g. custom/generic provider)', () => {
    const line = usageSummary(usage, 'generic-groq', 'llama-3.1-70b', [opus])
    expect(line).toBe('1.2K in · 340 out')
  })

  it('omits cost when provider is undefined', () => {
    const line = usageSummary(usage, undefined, 'claude-opus-4-8', [opus])
    expect(line).toBe('1.2K in · 340 out')
  })
})
