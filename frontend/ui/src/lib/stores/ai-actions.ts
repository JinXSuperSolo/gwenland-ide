import { get } from 'svelte/store'
import { activeDoc, activeSelection } from '../editor/active-editor'
import { readFile } from '../tauri/commands'
import { aiChat, setUnsentInput } from './ai-chat'
import { isEditorTab, tabs } from './tabs'

function activePath(): string | null {
  const state = get(tabs)
  const tab = state.tabs.find((candidate) => candidate.id === state.activeId)
  return tab && isEditorTab(tab) && tab.path ? tab.path : null
}

function openAiWith(prompt: string): void {
  aiChat.update((s) => ({ ...s, isOpen: true, unsentInput: prompt }))
}

function fenced(path: string | null, text: string): string {
  return `File: ${path ?? 'selection'}\n\n\`\`\`\n${text}\n\`\`\``
}

export function explainSelection(): void {
  const text = activeSelection() ?? activeDoc() ?? ''
  if (!text.trim()) return
  openAiWith(`Explain this code clearly and point out important behavior.\n\n${fenced(activePath(), text)}`)
}

export function fixSelection(): void {
  const text = activeSelection() ?? activeDoc() ?? ''
  if (!text.trim()) return
  openAiWith(`Fix bugs or edge cases in this code. Reply with a concise explanation and a unified diff if changes are needed.\n\n${fenced(activePath(), text)}`)
}

export function generateTestsForSelection(): void {
  const text = activeSelection() ?? activeDoc() ?? ''
  if (!text.trim()) return
  openAiWith(`Generate focused tests for this code. Prefer the existing test style and include only useful cases.\n\n${fenced(activePath(), text)}`)
}

export async function explainFile(path: string): Promise<void> {
  const content = await readFile(path).catch(() => '')
  openAiWith(`Explain this file and summarize its important responsibilities.\n\n${fenced(path, content)}`)
}

export async function refactorFile(path: string): Promise<void> {
  const content = await readFile(path).catch(() => '')
  openAiWith(`Suggest a safe refactor for this file. Reply with a concise rationale and a unified diff if edits are needed.\n\n${fenced(path, content)}`)
}
