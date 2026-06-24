/**
 * Single entry point that loads every feature area's context actions into the
 * shared registry at app init. New action modules (editor, terminal, git, …)
 * register their set here — the menu shell and panels never import action
 * modules directly.
 */
import { registerFileActions } from '../actions/fileActions'
import { registerWorkspaceActions } from '../actions/workspaceActions'
import { registerEditorActions } from '../actions/editorActions'
import { registerTerminalActions } from '../actions/terminalActions'
import { registerPanelActions } from '../actions/panelActions'
import { registerAiChatActions } from '../actions/aiChatActions'
import { registerGlobalActions } from '../actions/globalActions'

export function registerContextActions(): void {
  registerFileActions()
  registerWorkspaceActions()
  registerEditorActions()
  registerTerminalActions()
  registerPanelActions()
  registerAiChatActions()
  registerGlobalActions()
}
