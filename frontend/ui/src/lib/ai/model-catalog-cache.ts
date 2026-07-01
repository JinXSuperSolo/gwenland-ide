import { aiModelCatalog, type ModelEntry } from '../tauri/commands'

/**
 * Process-lifetime cache for the static model catalog. The catalog is
 * read-only backend data (`ai_model_catalog`) that never changes without an
 * app restart, so every caller shares one fetch instead of each `AiMessage`
 * instance re-requesting it over IPC.
 */
let cached: Promise<ModelEntry[]> | null = null

export function cachedModelCatalog(): Promise<ModelEntry[]> {
  if (!cached) {
    cached = aiModelCatalog().catch((e) => {
      cached = null // allow retry on next call after a transient failure
      throw e
    })
  }
  return cached
}
