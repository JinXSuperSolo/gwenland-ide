import { writable } from 'svelte/store'

export interface ConfirmDialogState {
  open: boolean
  title: string
  message: string
  confirmLabel: string
  danger: boolean
}

const closed: ConfirmDialogState = {
  open: false,
  title: '',
  message: '',
  confirmLabel: 'Confirm',
  danger: false,
}

export const confirmDialog = writable<ConfirmDialogState>(closed)

let resolver: ((value: boolean) => void) | null = null

export interface ConfirmOptions {
  title: string
  message: string
  confirmLabel?: string
  danger?: boolean
}

/** Show a yes/no dialog. Resolves to true if the user clicks the confirm button. */
export function openConfirm(opts: ConfirmOptions): Promise<boolean> {
  if (resolver) {
    resolver(false)
    resolver = null
  }
  confirmDialog.set({
    open: true,
    title: opts.title,
    message: opts.message,
    confirmLabel: opts.confirmLabel ?? 'Confirm',
    danger: opts.danger ?? false,
  })
  return new Promise((resolve) => {
    resolver = resolve
  })
}

export function acceptConfirm(): void {
  const r = resolver
  resolver = null
  confirmDialog.set(closed)
  r?.(true)
}

export function cancelConfirm(): void {
  const r = resolver
  resolver = null
  confirmDialog.set(closed)
  r?.(false)
}
