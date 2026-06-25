// M13 GWEN-339 — web search helper for AI self-search.
//
// Reuses browser fetch + stripHtml from mention-providers.ts.
// Zero new npm dependencies. Returns a capped plain-text snippet or an
// '[unavailable]' string on any failure (network, parse, empty result).

import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { aiSearchResult } from '../tauri/commands'
import { stripHtml } from '../stores/mention-providers'

// Approximate token cap: ~500 tokens × 4 chars/token.
const SEARCH_CHAR_CAP = 2000

/** Fetch a search result page and return stripped, capped text. */
async function fetchSearchText(query: string): Promise<string> {
  // Use DuckDuckGo HTML endpoint — no API key required.
  const url = `https://html.duckduckgo.com/html/?q=${encodeURIComponent(query)}`
  const res = await fetch(url, {
    headers: { 'User-Agent': 'GwenLandIDE/1.0 (search helper)' },
  })
  if (!res.ok) return ''

  const html = await res.text()
  const text = stripHtml(html)

  // Cap to budget, breaking at a line boundary.
  if (text.length <= SEARCH_CHAR_CAP) return text
  const truncated = text.slice(0, SEARCH_CHAR_CAP)
  const lastNewline = truncated.lastIndexOf('\n')
  return lastNewline > 0 ? truncated.slice(0, lastNewline) : truncated
}

/**
 * Register the `ai://search_request` listener. When the backend detects the
 * search trigger phrase it emits this event; we fetch the web result and call
 * `ai_search_result` to resume the stream.
 *
 * Returns an unlisten function — call it to deregister when the panel unmounts.
 */
export async function initSearchListener(): Promise<UnlistenFn> {
  return listen<{ stream_id: string; query: string }>(
    'ai://search_request',
    async ({ payload }) => {
      const { stream_id, query } = payload
      let result = '[unavailable]'
      try {
        const text = await fetchSearchText(query)
        if (text.trim().length > 0) result = text
      } catch {
        // silent — result stays '[unavailable]'
      }
      try {
        await aiSearchResult(stream_id, result)
      } catch {
        // command may fail if the stream was cancelled; ignore
      }
    },
  )
}
