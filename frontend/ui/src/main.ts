import { mount } from 'svelte'
import './styles/tokens.css'
import './styles/base.css'
import './styles/animations.css'
import './styles/editor.css'
import { initSettings } from './lib/stores/settings'
import { registerCommands } from './lib/commands/registry'
import { registerContextActions } from './lib/context-menu/registerActions'
import { initLsp } from './lib/stores/lsp'
import { initGit } from './lib/stores/git'
import { initAppFocus } from './lib/stores/app-focus'
import App from './App.svelte'

// Apply persisted theme before first paint; register the command/shortcut set
// and the context-menu action registry (M9); start listening for LSP
// diagnostics/status events (M6). initAppFocus() must run before initGit() so
// the background-throttle state is live when git polling starts.
initSettings()
registerCommands()
registerContextActions()
initLsp()
initAppFocus()
initGit()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
