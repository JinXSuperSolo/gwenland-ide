import { writable } from 'svelte/store'

/** Transient UI surface visibility: command palette + settings page. */
export const paletteOpen = writable(false)
export const settingsOpen = writable(false)
export const paletteInitialQuery = writable('')

export function openPalette(initialQuery = ''): void {
  paletteInitialQuery.set(initialQuery)
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

export const aboutOpen = writable(false)
export function openAbout(): void { aboutOpen.set(true) }
export function closeAbout(): void { aboutOpen.set(false) }

export const changelogOpen = writable(false)
export function openChangelog(): void { changelogOpen.set(true) }
export function closeChangelog(): void { changelogOpen.set(false) }
