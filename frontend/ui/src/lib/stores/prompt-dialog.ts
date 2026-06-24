import { writable } from 'svelte/store'

/**
 * A tiny promise-based text-input modal. `window.prompt` is unavailable in the
 * Tauri (WebView2) shell, so name entry for New File / New Folder / Rename goes
 * through this instead. One dialog at a time; opening a second resolves the
 * first as cancelled. Mirrors the native `confirm()` pattern already used for
 * destructive confirmations — small, self-owned, no new dependency.
 */
export interface PromptDialogState {
  open: boolean
  title: string
  label: string
  value: string
  placeholder: string
  confirmLabel: string
}

export interface PromptOptions {
  title: string
  label?: string
  initialValue?: string
  placeholder?: string
  confirmLabel?: string
}

const closed: PromptDialogState = {
  open: false,
  title: '',
  label: '',
  value: '',
  placeholder: '',
  confirmLabel: 'OK',
}

export const promptDialog = writable<PromptDialogState>(closed)

let resolver: ((value: string | null) => void) | null = null

/** Open the prompt; resolves to the entered (trimmed) string, or null if cancelled. */
export function openPrompt(opts: PromptOptions): Promise<string | null> {
  // Supersede any in-flight prompt as cancelled.
  if (resolver) {
    resolver(null)
    resolver = null
  }
  promptDialog.set({
    open: true,
    title: opts.title,
    label: opts.label ?? '',
    value: opts.initialValue ?? '',
    placeholder: opts.placeholder ?? '',
    confirmLabel: opts.confirmLabel ?? 'OK',
  })
  return new Promise((resolve) => {
    resolver = resolve
  })
}

/** Confirm with `value` (caller passes the trimmed input). */
export function confirmPrompt(value: string): void {
  const r = resolver
  resolver = null
  promptDialog.set(closed)
  r?.(value)
}

/** Dismiss without a value. */
export function cancelPrompt(): void {
  const r = resolver
  resolver = null
  promptDialog.set(closed)
  r?.(null)
}
