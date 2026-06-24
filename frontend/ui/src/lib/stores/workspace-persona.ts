import { writable, get } from 'svelte/store'
import { readFile, writeFile, createDir } from '../tauri/commands'
import { workspace } from './workspace'

/**
 * Per-workspace AI persona + system prompt (GWEN-334).
 *
 * The config lives in `.gwenland/GwenLand.md` (the same file `/setup` writes).
 * This module owns the canonical type, the Markdown parse/serialize round-trip,
 * and the persona store. Tauri reads/writes go through `loadPersona`/
 * `savePersona`; the pure parse/serialize functions are unit-tested and have no
 * I/O, matching the store/setup split used by slash-commands and mentions.
 *
 * The effective system prompt prefix is assembled in `../ai/persona-setup.ts`
 * and layered on top of the engine's base protocol prompt — this module only
 * holds the raw, user-authored values.
 */

/** Tone presets offered in the `/persona` picker. */
export type Tone = 'professional' | 'casual' | 'teacher' | 'silent'

export interface TonePreset {
  id: Tone
  label: string
  /** Short blurb shown in the picker. */
  hint: string
  /** Instruction line injected into the system prompt for this tone. */
  directive: string
}

export const TONE_PRESETS: TonePreset[] = [
  {
    id: 'professional',
    label: 'Professional',
    hint: 'Formal, concise, no filler',
    directive: 'Adopt a formal, professional tone. Be concise and precise; no filler.',
  },
  {
    id: 'casual',
    label: 'Casual',
    hint: 'Relaxed, friendly, short answers',
    directive: 'Adopt a relaxed, friendly tone. Keep answers short and approachable.',
  },
  {
    id: 'teacher',
    label: 'Teacher',
    hint: 'Explain everything step by step',
    directive: 'Take a teaching tone. Explain your reasoning step by step so the user learns.',
  },
  {
    id: 'silent',
    label: 'Silent',
    hint: 'Code only, zero explanation',
    directive: 'Respond with code only. Provide zero prose explanation unless explicitly asked.',
  },
]

export function tonePreset(id: Tone): TonePreset {
  return TONE_PRESETS.find((t) => t.id === id) ?? TONE_PRESETS[1]
}

/** The parsed AI Persona section. */
export interface Persona {
  name: string
  tone: Tone
  /** Free-text language preference (e.g. "Indonesian + English mix"). */
  language: string
  /** Free-text focus (e.g. "code + architecture"). */
  focus: string
}

/**
 * The full per-workspace config parsed from GwenLand.md. `systemPrompt` and
 * `customInstructions` are user-authored; empty values mean "use defaults".
 */
export interface PersonaConfig {
  persona: Persona
  /** Raw System Prompt section text ('' = fall back to the default). */
  systemPrompt: string
  /** Custom Instructions bullet lines (without the leading `- `). */
  customInstructions: string[]
}

/** The default GwenLand system prompt (fallback when none is configured). */
export const DEFAULT_SYSTEM_PROMPT = `You are GwenLand AI, a local-first coding assistant built into GwenLand IDE.

You help with code, architecture, debugging, and explanations.

Be concise and practical.`

/** The default persona name shown when no workspace config exists. */
export const DEFAULT_PERSONA_NAME = 'GwenLand AI'

/** A fresh, empty config (no workspace file, or a brand-new one). */
export function defaultConfig(): PersonaConfig {
  return {
    persona: { name: DEFAULT_PERSONA_NAME, tone: 'casual', language: '', focus: '' },
    systemPrompt: '',
    customInstructions: [],
  }
}

// --- Markdown parse / serialize --------------------------------------------

/** Section bodies keyed by their `## Heading` (lowercased, trimmed). */
function splitSections(md: string): Record<string, string> {
  const out: Record<string, string> = {}
  // Split on level-2 headings, keeping the heading with its body.
  const parts = md.split(/^##\s+/m)
  for (const part of parts) {
    const nl = part.indexOf('\n')
    if (nl === -1) continue
    const heading = part.slice(0, nl).trim().toLowerCase()
    const body = part.slice(nl + 1)
    if (heading) out[heading] = body
  }
  return out
}

/** Treat `/setup`'s placeholder text as "empty so defaults apply". */
function isPlaceholder(s: string): boolean {
  const t = s.trim()
  return t === '' || /^\[.*\]$/.test(t)
}

/** Read a `- Key: value` bullet (case-insensitive key) from a section body. */
function bulletValue(body: string, key: string): string {
  const re = new RegExp(`^[-*]\\s*${key}\\s*:\\s*(.+)$`, 'im')
  const m = body.match(re)
  return m ? m[1].trim() : ''
}

function parseTone(raw: string): Tone {
  const t = raw.trim().toLowerCase()
  if (t === 'professional' || t === 'casual' || t === 'teacher' || t === 'silent') return t
  return 'casual'
}

/**
 * Parse a GwenLand.md document into a `PersonaConfig`. Tolerant of missing
 * sections, the canonical format, and `/setup`'s `[placeholder]` lines (treated
 * as empty). Always returns a usable config — never throws.
 */
export function parseGwenLandMd(md: string): PersonaConfig {
  const config = defaultConfig()
  if (!md || !md.trim()) return config
  const sections = splitSections(md)

  // ── AI Persona ──
  const personaBody = sections['ai persona'] ?? ''
  if (personaBody && !isPlaceholder(personaBody)) {
    const name = bulletValue(personaBody, 'name')
    if (name && !isPlaceholder(name)) config.persona.name = name
    const tone = bulletValue(personaBody, 'tone')
    if (tone) config.persona.tone = parseTone(tone)
    const language = bulletValue(personaBody, 'language')
    if (language && !isPlaceholder(language)) config.persona.language = language
    const focus = bulletValue(personaBody, 'focus')
    if (focus && !isPlaceholder(focus)) config.persona.focus = focus
  }

  // ── System Prompt ── (free-form block; strip trailing section noise)
  const sysBody = (sections['system prompt'] ?? '').trim()
  if (sysBody && !isPlaceholder(sysBody)) config.systemPrompt = sysBody

  // ── Custom Instructions ── (bullet list)
  const ciBody = sections['custom instructions'] ?? ''
  if (ciBody && !isPlaceholder(ciBody)) {
    config.customInstructions = ciBody
      .split('\n')
      .map((l) => l.replace(/^[-*]\s+/, '').trim())
      .filter((l) => l.length > 0 && !isPlaceholder(l))
  }

  return config
}

/**
 * Serialize a `PersonaConfig` back into the canonical GwenLand.md sections. When
 * an existing document is supplied, its non-persona sections (Workspace,
 * Workspace Context, etc.) are preserved and only the three managed sections are
 * rewritten; otherwise a minimal canonical document is produced.
 */
export function serializeGwenLandMd(config: PersonaConfig, existing?: string): string {
  const persona = renderPersonaSection(config.persona)
  const system = renderSystemSection(config.systemPrompt)
  const custom = renderCustomSection(config.customInstructions)

  if (existing && existing.trim()) {
    let out = existing
    out = replaceSection(out, 'AI Persona', persona)
    out = replaceSection(out, 'System Prompt', system)
    out = replaceSection(out, 'Custom Instructions', custom)
    return out
  }

  return `# GwenLand Workspace Config

## AI Persona
${persona}

## System Prompt
${system}

## Custom Instructions
${custom}
`
}

function renderPersonaSection(p: Persona): string {
  return [
    `- Name: ${p.name}`,
    `- Tone: ${p.tone}`,
    `- Language: ${p.language || '[unset]'}`,
    `- Focus: ${p.focus || '[unset]'}`,
  ].join('\n')
}

function renderSystemSection(prompt: string): string {
  return prompt.trim() || '[default empty — user fills in]'
}

function renderCustomSection(lines: string[]): string {
  const real = lines.map((l) => l.trim()).filter(Boolean)
  return real.length ? real.map((l) => `- ${l}`).join('\n') : '[default empty — user fills in]'
}

/**
 * Replace a `## Heading` section body in `md` with `body`, or append the section
 * when it doesn't exist. Section ends at the next `## ` or `# ` heading, the
 * `---` footer, or end of file.
 */
function replaceSection(md: string, heading: string, body: string): string {
  const re = new RegExp(
    `(^##\\s+${escapeRe(heading)}\\s*\\n)([\\s\\S]*?)(?=^##\\s+|^#\\s+|^---\\s*$|$(?![\\s\\S]))`,
    'im'
  )
  if (re.test(md)) {
    return md.replace(re, (_m, head: string) => `${head}${body}\n\n`)
  }
  // Append before a trailing `---` footer if present, else at the end.
  const section = `## ${heading}\n${body}\n`
  const footer = md.match(/^---[\s\S]*$/m)
  if (footer) return md.replace(/^---[\s\S]*$/m, `${section}\n${footer[0]}`)
  return `${md.trimEnd()}\n\n${section}`
}

function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

// --- Store + I/O -----------------------------------------------------------

/** The active workspace persona config (defaults until a workspace loads). */
export const persona = writable<PersonaConfig>(defaultConfig())

/** Convenience snapshot read. */
export function personaState(): PersonaConfig {
  return get(persona)
}

/** Path to the workspace config file under a project root. */
function gwenLandPath(root: string): string {
  return `${root}/.gwenland/GwenLand.md`
}

/** Raw text of the open workspace's GwenLand.md, or null when absent/no project. */
async function readGwenLandMd(): Promise<string | null> {
  const root = get(workspace).folderPath
  if (!root) return null
  try {
    return await readFile(gwenLandPath(root))
  } catch {
    return null // not found / unreadable → defaults
  }
}

/**
 * Load the open workspace's persona config into the store. Falls back to
 * defaults when there's no project or no GwenLand.md. Returns the config it set.
 * Changes take effect on the next message — no restart needed.
 */
export async function loadPersona(): Promise<PersonaConfig> {
  const md = await readGwenLandMd()
  const config = md ? parseGwenLandMd(md) : defaultConfig()
  persona.set(config)
  return config
}

/** Reset the store to defaults (called when the workspace closes). */
export function resetPersona(): void {
  persona.set(defaultConfig())
}

/**
 * Persist `config` to the open workspace's GwenLand.md (preserving any non-managed
 * sections) and update the store. Creates `.gwenland/` if needed. Rejects (so the
 * caller can surface an error) when no project is open or the write fails.
 */
export async function savePersona(config: PersonaConfig): Promise<void> {
  const root = get(workspace).folderPath
  if (!root) throw new Error('Open a project folder first.')
  const existing = await readGwenLandMd()
  const md = serializeGwenLandMd(config, existing ?? undefined)
  // Ensure `.gwenland/` exists (create_dir rejects if present — ignore that).
  try {
    await createDir(`${root}/.gwenland`, root)
  } catch {
    /* already exists */
  }
  await writeFile(gwenLandPath(root), md)
  persona.set(config)
}
