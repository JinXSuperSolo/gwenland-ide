import { derived, get } from 'svelte/store'
import { comboFromEvent, handleGlobalKeydown } from '../commands/keybinding-handler'
import {
  commandCategory,
  commands as registryCommands,
  filterCommands as filterRegistryCommands,
  registerCommand as registerRegistryCommand,
  registerCommands as registerRegistryCommands,
  shortcutFor,
  type Command as RegistryCommand,
} from '../commands/registry'

/**
 * Back-compat facade for older M2/M9 call sites. New command work lives in
 * `lib/commands/registry.ts`; this shape is kept so context-menu shortcut
 * lookups and older action modules continue to compile.
 */
export interface Command {
  id: string
  label: string
  keys: string[]
  action: () => void
}

function toLegacy(command: RegistryCommand): Command {
  const keys = command.defaultKeybinding
    ? command.defaultKeybinding.split('/').map((key) => key.trim()).filter(Boolean)
    : []
  return {
    id: command.id,
    label: command.title,
    keys,
    action: () => void command.handler(),
  }
}

export const commands = derived(registryCommands, (list) => list.map(toLegacy))

export function registerCommand(
  id: string,
  label: string,
  keys: string[],
  action: () => void,
): void {
  registerRegistryCommand({
    id,
    title: label,
    category: commandCategory(id) || id.split('.')[0] || 'Command',
    defaultKeybinding: keys.join(' / ') || undefined,
    handler: action,
  })
}

export function buildComboString(e: KeyboardEvent): string {
  return comboFromEvent(e)
}

export function dispatchShortcut(e: KeyboardEvent): boolean {
  return handleGlobalKeydown(e)
}

export function filterCommands(query: string): Command[] {
  return filterRegistryCommands(query).map(toLegacy)
}

export { shortcutFor, commandCategory, registerRegistryCommands }

export function currentCommands(): Command[] {
  return get(commands)
}
