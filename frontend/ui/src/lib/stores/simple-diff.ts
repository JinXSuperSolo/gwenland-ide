import { writable } from 'svelte/store'

export interface SimpleDiffState {
  open: boolean
  title: string
  leftLabel: string
  rightLabel: string
  left: string
  right: string
}

const initial: SimpleDiffState = {
  open: false,
  title: '',
  leftLabel: '',
  rightLabel: '',
  left: '',
  right: '',
}

export const simpleDiff = writable<SimpleDiffState>(initial)

export function openSimpleDiff(input: Omit<SimpleDiffState, 'open'>): void {
  simpleDiff.set({ ...input, open: true })
}

export function closeSimpleDiff(): void {
  simpleDiff.update((s) => ({ ...s, open: false }))
}
