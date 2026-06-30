import { writable } from 'svelte/store'
import {
  loadEngineSettings,
  saveEngineSettings,
  type EngineSettings,
  type LspLanguage,
  type LspOnboardingSettings,
} from '../tauri/commands'

export const DEFAULT_LSP_ONBOARDING: LspOnboardingSettings = {
  rust: false,
  typescript: false,
  javascript: false,
  python: false,
}

interface LspOnboardingState {
  loaded: boolean
  dismissed: LspOnboardingSettings
}

export const lspOnboarding = writable<LspOnboardingState>({
  loaded: false,
  dismissed: { ...DEFAULT_LSP_ONBOARDING },
})

export function normalizeLspOnboarding(settings: EngineSettings): EngineSettings {
  settings.lsp.onboarding = {
    ...DEFAULT_LSP_ONBOARDING,
    ...(settings.lsp.onboarding ?? {}),
  }
  return settings
}

export async function loadLspOnboarding(): Promise<EngineSettings> {
  const settings = normalizeLspOnboarding(await loadEngineSettings())
  lspOnboarding.set({ loaded: true, dismissed: { ...settings.lsp.onboarding } })
  return settings
}

export async function dismissLspOnboarding(language: LspLanguage): Promise<EngineSettings> {
  const settings = normalizeLspOnboarding(await loadEngineSettings())
  settings.lsp.onboarding[language] = true
  await saveEngineSettings(settings)
  lspOnboarding.set({ loaded: true, dismissed: { ...settings.lsp.onboarding } })
  return settings
}

export async function resetLspOnboarding(): Promise<EngineSettings> {
  const settings = normalizeLspOnboarding(await loadEngineSettings())
  settings.lsp.onboarding = { ...DEFAULT_LSP_ONBOARDING }
  await saveEngineSettings(settings)
  lspOnboarding.set({ loaded: true, dismissed: { ...settings.lsp.onboarding } })
  return settings
}
