import { describe, it, expect } from 'vitest'
import {
  parseGwenLandMd,
  serializeGwenLandMd,
  defaultConfig,
  DEFAULT_PERSONA_NAME,
  type PersonaConfig,
} from './workspace-persona'

// A canonical document matching the GWEN-334 format (and /setup output shape).
const CANONICAL = `# GwenLand Workspace Config

## Workspace
- Root: /path/to/project
- Language: Rust
- Framework: Tauri 2 + Svelte
- Key files: src-tauri/src/main.rs, frontend/src/App.svelte

## AI Persona
- Name: Gwen
- Tone: casual
- Language: Indonesian + English mix
- Focus: code + architecture

## System Prompt
You are Gwen, a local-first AI coding assistant.

## Custom Instructions
- Always prefer Rust idioms over unsafe code
- Keep binary size in mind
- No heavy dependencies

## Workspace Context
- What this project does: GwenLand IDE — local-first AI code editor
- Last updated: 2026-06-24
`

describe('parseGwenLandMd', () => {
  it('parses the canonical document', () => {
    const c = parseGwenLandMd(CANONICAL)
    expect(c.persona.name).toBe('Gwen')
    expect(c.persona.tone).toBe('casual')
    expect(c.persona.language).toBe('Indonesian + English mix')
    expect(c.persona.focus).toBe('code + architecture')
    expect(c.systemPrompt).toBe('You are Gwen, a local-first AI coding assistant.')
    expect(c.customInstructions).toEqual([
      'Always prefer Rust idioms over unsafe code',
      'Keep binary size in mind',
      'No heavy dependencies',
    ])
  })

  it('returns defaults for empty/blank input', () => {
    expect(parseGwenLandMd('')).toEqual(defaultConfig())
    expect(parseGwenLandMd('   \n\n  ')).toEqual(defaultConfig())
  })

  it('treats /setup [placeholder] lines as empty (defaults apply)', () => {
    const md = `# GwenLand Workspace Config

## AI Persona
- Name: GwenLand AI
- Tone: casual
- Language: [unset]
- Focus: [unset]

## System Prompt
[default empty — user fills in]

## Custom Instructions
[default empty — user fills in]
`
    const c = parseGwenLandMd(md)
    expect(c.persona.name).toBe(DEFAULT_PERSONA_NAME)
    expect(c.persona.language).toBe('')
    expect(c.persona.focus).toBe('')
    expect(c.systemPrompt).toBe('')
    expect(c.customInstructions).toEqual([])
  })

  it('falls back to casual for an unknown tone', () => {
    const md = `## AI Persona\n- Name: X\n- Tone: grumpy\n`
    expect(parseGwenLandMd(md).persona.tone).toBe('casual')
  })

  it('tolerates missing sections', () => {
    const md = `## AI Persona\n- Name: Solo\n`
    const c = parseGwenLandMd(md)
    expect(c.persona.name).toBe('Solo')
    expect(c.systemPrompt).toBe('')
    expect(c.customInstructions).toEqual([])
  })
})

describe('serializeGwenLandMd', () => {
  it('produces a minimal canonical document when no existing file', () => {
    const config: PersonaConfig = {
      persona: { name: 'Gwen', tone: 'teacher', language: 'EN', focus: 'tests' },
      systemPrompt: 'Be helpful.',
      customInstructions: ['One', 'Two'],
    }
    const md = serializeGwenLandMd(config)
    expect(md).toContain('## AI Persona')
    expect(md).toContain('- Name: Gwen')
    expect(md).toContain('- Tone: teacher')
    expect(md).toContain('## System Prompt')
    expect(md).toContain('Be helpful.')
    expect(md).toContain('- One')
    expect(md).toContain('- Two')
  })

  it('uses placeholders for empty system prompt / instructions', () => {
    const md = serializeGwenLandMd(defaultConfig())
    expect(md).toContain('[default empty — user fills in]')
  })

  it('preserves non-managed sections when editing an existing document', () => {
    const config = parseGwenLandMd(CANONICAL)
    config.persona.name = 'Renamed'
    const md = serializeGwenLandMd(config, CANONICAL)
    // Managed change applied…
    expect(md).toContain('- Name: Renamed')
    // …and the Workspace + Workspace Context sections survive.
    expect(md).toContain('## Workspace')
    expect(md).toContain('- Root: /path/to/project')
    expect(md).toContain('## Workspace Context')
    expect(md).toContain('local-first AI code editor')
  })
})

describe('round-trip', () => {
  it('parse → serialize → parse is stable for the managed fields', () => {
    const first = parseGwenLandMd(CANONICAL)
    const md = serializeGwenLandMd(first, CANONICAL)
    const second = parseGwenLandMd(md)
    expect(second.persona).toEqual(first.persona)
    expect(second.systemPrompt).toBe(first.systemPrompt)
    expect(second.customInstructions).toEqual(first.customInstructions)
  })
})
