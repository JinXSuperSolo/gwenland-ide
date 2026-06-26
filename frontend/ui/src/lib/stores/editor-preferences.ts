import { writable } from 'svelte/store'

export interface EditorPreferences {
  previewEditors: boolean
  editorMinimap: boolean
  terminalMinimap: boolean
  markdownPreview: boolean
}

const STORAGE_KEY = 'gwen.editor.preferences.v1'
const DEFAULTS: EditorPreferences = {
  previewEditors: false,
  editorMinimap: false,
  terminalMinimap: false,
  markdownPreview: false,
}

function load(): EditorPreferences {
  if (typeof localStorage === 'undefined') return { ...DEFAULTS }
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}')
    return {
      previewEditors:
        typeof saved.previewEditors === 'boolean'
          ? saved.previewEditors
          : DEFAULTS.previewEditors,
      editorMinimap:
        typeof saved.editorMinimap === 'boolean'
          ? saved.editorMinimap
          : DEFAULTS.editorMinimap,
      terminalMinimap:
        typeof saved.terminalMinimap === 'boolean'
          ? saved.terminalMinimap
          : DEFAULTS.terminalMinimap,
      markdownPreview:
        typeof saved.markdownPreview === 'boolean'
          ? saved.markdownPreview
          : DEFAULTS.markdownPreview,
    }
  } catch {
    return { ...DEFAULTS }
  }
}

function persist(value: EditorPreferences): void {
  if (typeof localStorage === 'undefined') return
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(value))
  } catch {
    /* storage unavailable; non-fatal */
  }
}

export const editorPreferences = writable<EditorPreferences>(load())

editorPreferences.subscribe((value) => persist(value))

export function togglePreviewEditors(): void {
  editorPreferences.update((value) => ({
    ...value,
    previewEditors: !value.previewEditors,
  }))
}

export function toggleEditorMinimap(): void {
  editorPreferences.update((value) => ({
    ...value,
    editorMinimap: !value.editorMinimap,
  }))
}

export function toggleTerminalMinimap(): void {
  editorPreferences.update((value) => ({
    ...value,
    terminalMinimap: !value.terminalMinimap,
  }))
}

export function toggleMarkdownPreview(): void {
  editorPreferences.update((value) => ({
    ...value,
    markdownPreview: !value.markdownPreview,
  }))
}
