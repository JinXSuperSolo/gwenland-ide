import { writable } from 'svelte/store'

export interface GitGraphWindowState {
  open: boolean
  workspacePath: string | null
  x: number
  y: number
  width: number
  height: number
  maximized: boolean
}

const initial: GitGraphWindowState = {
  open: false,
  workspacePath: null,
  x: 96,
  y: 64,
  width: 980,
  height: 620,
  maximized: false,
}

export const gitGraphWindow = writable<GitGraphWindowState>(initial)

export function openGitGraphWindow(workspacePath: string): void {
  gitGraphWindow.update((state) => ({
    ...state,
    open: true,
    workspacePath,
  }))
}

export function closeGitGraphWindow(): void {
  gitGraphWindow.update((state) => ({
    ...state,
    open: false,
    maximized: false,
  }))
}

export function setGitGraphWindowBounds(
  bounds: Partial<Pick<GitGraphWindowState, 'x' | 'y' | 'width' | 'height'>>,
): void {
  gitGraphWindow.update((state) => ({
    ...state,
    ...bounds,
  }))
}

export function toggleGitGraphWindowMaximized(): void {
  gitGraphWindow.update((state) => ({
    ...state,
    maximized: !state.maximized,
  }))
}
