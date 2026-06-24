import type { Action } from 'svelte/action'
import { closeContextMenu } from './contextMenuStore'

/**
 * Keyboard navigation for the menu shell (Requirement 3), attached to the menu
 * container as `use:contextMenuKeyNav`. Keydown from a focused item bubbles to
 * the container, so a single listener drives the whole menu:
 *
 *   ArrowDown / ArrowUp  → focus next / prev enabled item (wraps)
 *   Home / End           → focus first / last enabled item
 *   Escape               → close the menu
 *   Enter / Space        → handled natively by the focused <button>
 *
 * Disabled items carry the native `disabled` attribute, so the `:not(:disabled)`
 * query skips them automatically.
 */
export const contextMenuKeyNav: Action<HTMLElement> = (node) => {
  function items(): HTMLButtonElement[] {
    return Array.from(node.querySelectorAll<HTMLButtonElement>('[data-cm-item]:not(:disabled)'))
  }

  function currentIndex(list: HTMLButtonElement[]): number {
    return list.indexOf(document.activeElement as HTMLButtonElement)
  }

  function focusAt(list: HTMLButtonElement[], index: number): void {
    if (list.length === 0) return
    const wrapped = ((index % list.length) + list.length) % list.length
    list[wrapped].focus()
  }

  function onKeydown(e: KeyboardEvent): void {
    switch (e.key) {
      case 'ArrowDown': {
        e.preventDefault()
        const list = items()
        focusAt(list, currentIndex(list) + 1)
        break
      }
      case 'ArrowUp': {
        e.preventDefault()
        const list = items()
        const idx = currentIndex(list)
        focusAt(list, idx <= 0 ? list.length - 1 : idx - 1)
        break
      }
      case 'Home': {
        e.preventDefault()
        focusAt(items(), 0)
        break
      }
      case 'End': {
        e.preventDefault()
        const list = items()
        focusAt(list, list.length - 1)
        break
      }
      case 'Escape': {
        e.preventDefault()
        closeContextMenu()
        break
      }
    }
  }

  node.addEventListener('keydown', onKeydown)
  return {
    destroy() {
      node.removeEventListener('keydown', onKeydown)
    },
  }
}
