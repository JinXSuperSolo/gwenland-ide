import { get } from 'svelte/store'
import {
  persona,
  tonePreset,
  DEFAULT_SYSTEM_PROMPT,
  type PersonaConfig,
} from '../stores/workspace-persona'

/**
 * Persona → system-prompt assembly (GWEN-334). Turns the per-workspace
 * `PersonaConfig` into the `system_prefix` string the engine layers on TOP of
 * its base protocol prompt (which enforces the always-on <think> + review-first
 * diff format). Pure + Tauri-free; the store/I/O live in
 * `../stores/workspace-persona.ts`.
 */

/** True when the config has no user customization (use engine defaults). */
function isEmptyConfig(c: PersonaConfig): boolean {
  return (
    c.systemPrompt.trim() === '' &&
    c.customInstructions.length === 0 &&
    c.persona.name === 'GwenLand AI' &&
    c.persona.language.trim() === '' &&
    c.persona.focus.trim() === ''
  )
}

/**
 * Build the system-prompt prefix for a config. Returns '' when the config is
 * effectively empty (the engine then uses only its base prompt). Otherwise
 * composes: the system prompt (custom or default), persona identity/voice, and
 * any custom instructions.
 */
export function buildSystemPrefix(config: PersonaConfig): string {
  if (isEmptyConfig(config)) return ''

  const parts: string[] = []

  // 1. Base system prompt — the user's custom one, else the GwenLand default.
  parts.push(config.systemPrompt.trim() || DEFAULT_SYSTEM_PROMPT)

  // 2. Persona identity + voice.
  const p = config.persona
  const personaLines: string[] = []
  if (p.name && p.name !== 'GwenLand AI') personaLines.push(`Your name is ${p.name}.`)
  personaLines.push(tonePreset(p.tone).directive)
  if (p.language.trim()) personaLines.push(`Preferred language: ${p.language.trim()}.`)
  if (p.focus.trim()) personaLines.push(`Focus on: ${p.focus.trim()}.`)
  if (personaLines.length) parts.push(`Persona:\n${personaLines.join('\n')}`)

  // 3. Custom instructions.
  if (config.customInstructions.length) {
    parts.push(
      `Custom instructions:\n${config.customInstructions.map((l) => `- ${l}`).join('\n')}`
    )
  }

  return parts.join('\n\n')
}

/**
 * The active workspace's system prefix, read from the persona store. Passed as
 * `systemPrefix` to `ai_send`; '' means "engine default only".
 */
export function activeSystemPrefix(): string {
  return buildSystemPrefix(get(persona))
}

/**
 * Prepend the persona system-prompt block to a list of chat messages as a
 * leading `system` turn (used where the request is assembled message-first). For
 * the streaming `ai_send` path the prefix is passed as a dedicated field instead
 * — see `activeSystemPrefix`. No-op when the config is empty.
 */
export function applyPersonaToRequest<T extends { role: string; content: string }>(
  messages: T[],
  config: PersonaConfig
): T[] {
  const prefix = buildSystemPrefix(config)
  if (!prefix) return messages
  const systemTurn = { role: 'system', content: prefix } as T
  return [systemTurn, ...messages]
}
