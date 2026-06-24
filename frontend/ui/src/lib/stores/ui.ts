import { writable } from 'svelte/store'

/** Transient UI surface visibility: command palette + settings page. */
export const paletteOpen = writable(false)
export const settingsOpen = writable(false)

export function openPalette(): void {
  paletteOpen.set(true)
}
export function closePalette(): void {
  paletteOpen.set(false)
}
export function openSettings(): void {
  settingsOpen.set(true)
}
export function closeSettings(): void {
  settingsOpen.set(false)
}
export function toggleSettings(): void {
  settingsOpen.update((v) => !v)
}
