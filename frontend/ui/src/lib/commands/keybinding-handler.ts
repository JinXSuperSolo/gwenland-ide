import { allCommands, keybindingsFor, runCommand } from './registry'

const MODIFIER_KEYS = new Set(['Control', 'Meta', 'Alt', 'Shift'])
const KEY_FROM_CODE: Record<string, string> = {
  Backquote: '`',
  Backslash: '\\',
  Slash: '/',
  Comma: ',',
  Period: '.',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Semicolon: ';',
  Quote: "'",
}

function normalizedBaseKey(e: KeyboardEvent): string {
  if (e.code.startsWith('Key')) return e.code.slice(3).toUpperCase()
  if (e.code.startsWith('Digit')) return e.code.slice(5)
  if (KEY_FROM_CODE[e.code]) return KEY_FROM_CODE[e.code]
  if (e.key.startsWith('Arrow')) return e.key.slice('Arrow'.length)
  if (e.key.length === 1) return e.key.toUpperCase()
  return e.key
}

export function normalizeKeybinding(binding: string): string {
  const raw = binding
    .split('+')
    .map((part) => part.trim())
    .filter(Boolean)
  const key = raw.pop()
  if (!key) return ''
  const mods = new Set(raw.map((part) => (part === 'Cmd' ? 'Meta' : part)))
  const ordered = ['Ctrl', 'Meta', 'Shift', 'Alt'].filter((mod) => mods.has(mod))
  return [...ordered, key].join('+')
}

export function comboFromEvent(e: KeyboardEvent): string {
  const parts: string[] = []
  if (e.ctrlKey) parts.push('Ctrl')
  if (e.metaKey) parts.push('Meta')
  if (e.shiftKey) parts.push('Shift')
  if (e.altKey) parts.push('Alt')

  if (MODIFIER_KEYS.has(e.key)) return parts.join('+')
  parts.push(normalizedBaseKey(e))
  return parts.join('+')
}

function isReloadShortcut(e: KeyboardEvent): boolean {
  return (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && normalizedBaseKey(e) === 'R'
}

export function handleGlobalKeydown(e: KeyboardEvent): boolean {
  if (isReloadShortcut(e)) {
    e.preventDefault()
    e.stopPropagation()
    return true
  }

  const combo = comboFromEvent(e)
  const command = allCommands().find((entry) =>
    keybindingsFor(entry).some((binding) => normalizeKeybinding(binding) === combo),
  )
  if (!command) return false

  e.preventDefault()
  e.stopPropagation()
  void runCommand(command.id)
  return true
}
