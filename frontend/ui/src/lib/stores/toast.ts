import { writable } from 'svelte/store'

export type ToastKind = 'info' | 'success' | 'error'

export interface Toast {
  id: number
  message: string
  kind: ToastKind
}

let next = 0
export const toasts = writable<Toast[]>([])

export function toast(message: string, kind: ToastKind = 'info', durationMs = 3000): void {
  const id = ++next
  toasts.update((list) => [...list, { id, message, kind }])
  setTimeout(() => {
    toasts.update((list) => list.filter((t) => t.id !== id))
  }, durationMs)
}

export function dismissToast(id: number): void {
  toasts.update((list) => list.filter((t) => t.id !== id))
}
