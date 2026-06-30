import { mount } from 'svelte'
import './styles/tokens.css'
import './styles/global.css'
import './styles/animations.css'
import './styles/editor.css'
import { initSettings } from './lib/stores/settings'
import { registerCommands } from './lib/commands/registry'
import { registerContextActions } from './lib/context-menu/registerActions'
import { initLsp } from './lib/stores/lsp'
import { initGit } from './lib/stores/git'
import { initFsWatch } from './lib/stores/fs-watch'
import { applyLowEndClass } from './lib/stores/performance'
import { initAppFocus } from './lib/stores/app-focus'
import { initTreeInteraction } from './lib/stores/tree-interaction'
import App from './App.svelte'

// Apply persisted theme before first paint; register the command/shortcut set
// and the context-menu action registry (M9). Background services are deferred
// until after first paint so bundled startup has less work on the critical path.
initSettings()
registerCommands()
registerContextActions()
initAppFocus()
initTreeInteraction()
applyLowEndClass()

const app = mount(App, {
  target: document.getElementById('app')!,
})

function initBackgroundServices() {
  initLsp()
  initGit()
  initFsWatch()
}

requestAnimationFrame(() => {
  window.setTimeout(initBackgroundServices, 0)
})

export default app
