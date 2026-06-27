import { writable } from 'svelte/store'
import { expandPanel } from './panels'

/**
 * Which view the left sidebar shows (Wave 2 — GWEN-328 adds Source Control next
 * to the existing Explorer). The file-tree panel slot renders one of these.
 */
export type SidebarView = 'explorer' | 'search' | 'git'

export const sidebarView = writable<SidebarView>('explorer')

/** Switch the sidebar view and make sure the file-tree panel is expanded. */
export function showSidebarView(view: SidebarView): void {
  sidebarView.set(view)
  expandPanel('fileTree')
}
