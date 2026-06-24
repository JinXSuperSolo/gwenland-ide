import { get } from 'svelte/store'
import { registerCommand } from '../stores/commands'
import { workspace } from '../stores/workspace'
import { git, refreshGit } from '../stores/git'
import { openPrompt } from '../stores/prompt-dialog'
import {
  gitListBranches,
  gitCheckout,
  gitCreateBranch,
  gitDeleteBranch,
} from '../tauri/commands'

/**
 * Git command-palette actions (GWEN-331). These complement the status-bar branch
 * switcher with keyboard-driven entry points. Each is a no-op outside a git repo.
 */

function repoRoot(): string | null {
  const root = get(workspace).folderPath
  return root && get(git).isRepo ? root : null
}

async function checkoutBranch(): Promise<void> {
  const root = repoRoot()
  if (!root) return
  let branches: string[] = []
  try {
    branches = await gitListBranches(root)
  } catch {
    /* fall through to free-text */
  }
  const hint = branches.length ? ` (${branches.join(', ')})` : ''
  const name = await openPrompt({
    title: 'Checkout Branch',
    label: 'Branch name' + hint,
    placeholder: 'main',
  })
  if (!name) return
  try {
    await gitCheckout(root, name)
    await refreshGit()
  } catch (e) {
    alert(`Checkout failed: ${e}`)
  }
}

async function createBranch(): Promise<void> {
  const root = repoRoot()
  if (!root) return
  const name = await openPrompt({
    title: 'Create Branch',
    label: 'New branch name (spaces become hyphens)',
    placeholder: 'my-feature',
  })
  if (!name) return
  try {
    await gitCreateBranch(root, name)
    await refreshGit()
  } catch (e) {
    alert(`Create branch failed: ${e}`)
  }
}

async function deleteBranch(): Promise<void> {
  const root = repoRoot()
  if (!root) return
  const current = get(git).branch
  let branches: string[] = []
  try {
    branches = (await gitListBranches(root)).filter((b) => b !== current)
  } catch {
    /* fall through */
  }
  if (branches.length === 0) {
    alert('No other branches to delete.')
    return
  }
  const name = await openPrompt({
    title: 'Delete Branch',
    label: `Branch to delete (${branches.join(', ')})`,
    placeholder: branches[0],
  })
  if (!name) return
  if (name === current) {
    alert('Cannot delete the current branch.')
    return
  }
  if (!confirm(`Delete branch "${name}"? This cannot be undone.`)) return
  try {
    await gitDeleteBranch(root, name)
    await refreshGit()
  } catch (e) {
    alert(`Delete branch failed: ${e}`)
  }
}

/** Register the git palette commands (called at startup). */
export function registerGitCommands(): void {
  registerCommand('git.checkoutBranch', 'Git: Checkout Branch', [], () => void checkoutBranch())
  registerCommand('git.createBranch', 'Git: Create Branch', [], () => void createBranch())
  registerCommand('git.deleteBranch', 'Git: Delete Branch', [], () => void deleteBranch())
}
