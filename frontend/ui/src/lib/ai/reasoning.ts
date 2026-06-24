import type { ReasoningLevel } from '../stores/ai-chat'

/**
 * Reasoning-level helpers (Requirement 6). Pure, dependency-free, and testable.
 *
 * The reasoning selector is only shown for models we believe support thinking.
 * Wave 3 only governs UI visibility + stored state; Wave 4 decides whether the
 * level is actually sent, and only for providers whose adapter supports it.
 */

export const REASONING_LEVELS: { id: ReasoningLevel; label: string }[] = [
  { id: 'low', label: 'Low' },
  { id: 'medium', label: 'Medium' },
  { id: 'high', label: 'High' },
  { id: 'extra_high', label: 'Extra High' },
]

/** Anthropic: `claude-3-7-sonnet` and later families support extended thinking. */
function anthropicThinking(model: string): boolean {
  const m = model.toLowerCase()
  if (m.includes('claude-3-7')) return true
  // Claude 4.x+ families (opus/sonnet/haiku) — e.g. claude-opus-4-8.
  const major = /claude-(?:opus|sonnet|haiku)-(\d+)/.exec(m)?.[1]
  if (major) return Number(major) >= 4
  // Older ids (claude-3-5-*, claude-3-haiku, claude-2-*) are not thinking models.
  return false
}

/**
 * OpenAI-compatible local heuristic (Req 6.4, 6.5): treat ids containing
 * `deepseek-r1`, `qwen3`, or `qwq` as thinking-capable. This also serves as the
 * documented heuristic for configured generic/Ollama-compatible providers.
 */
function openaiCompatThinking(model: string): boolean {
  const m = model.toLowerCase()
  return m.includes('deepseek-r1') || m.includes('qwen3') || m.includes('qwq')
}

/** Whether the active provider/model should expose the reasoning selector. */
export function isThinkingCapable(provider: string, model: string): boolean {
  if (!model) return false
  if (provider === 'anthropic') return anthropicThinking(model)
  return openaiCompatThinking(model)
}
