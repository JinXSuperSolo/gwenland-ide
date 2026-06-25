import { get } from 'svelte/store'
import { historySaveEntry, type HistorySource } from '../tauri/commands'
import { workspace } from './workspace'

const SAVE_HISTORY_DELAY_MS = 2000
const timers = new Map<string, ReturnType<typeof setTimeout>>()

export function scheduleHistorySnapshot(
  filePath: string,
  content: string,
  source: HistorySource = 'save',
): void {
  const root = get(workspace).folderPath
  if (!root || !filePath) return
  const key = `${root}\n${filePath}`
  const existing = timers.get(key)
  if (existing) clearTimeout(existing)
  timers.set(
    key,
    setTimeout(() => {
      timers.delete(key)
      void historySaveEntry(root, filePath, content, source).catch(() => {})
    }, SAVE_HISTORY_DELAY_MS),
  )
}

export function createHistorySnapshot(
  filePath: string,
  content: string,
  source: HistorySource = 'manual',
): Promise<void> {
  const root = get(workspace).folderPath
  if (!root || !filePath) return Promise.resolve()
  return historySaveEntry(root, filePath, content, source).then(() => {})
}
