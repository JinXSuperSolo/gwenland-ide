/**
 * Chunk-safe `<think>...</think>` parser (Requirement 7.2).
 *
 * Streamed assistant text may open/close thinking tags at any byte boundary, so
 * a naive `indexOf` per chunk would miss tags split across chunks. This parser
 * keeps a tiny carry buffer holding any trailing substring that could be the
 * start of a tag, and routes text to either the thinking channel or the answer
 * channel. Raw tags never appear in either output (Req 7.6, 7.7).
 *
 * The same logic also handles structured provider thinking: adapters wrap
 * Anthropic/Ollama/DeepSeek reasoning in `<think>...</think>` within the text
 * stream, so this single parser separates reasoning for every provider.
 */

const OPEN = '<think>'
const CLOSE = '</think>'

export interface ThinkSplit {
  thinking: string
  answer: string
}

/** Length of the longest suffix of `s` that is a proper prefix of `tag`. */
function partialTailLen(s: string, tag: string): number {
  const max = Math.min(s.length, tag.length - 1)
  for (let len = max; len > 0; len--) {
    if (s.slice(s.length - len) === tag.slice(0, len)) return len
  }
  return 0
}

export interface ThinkParser {
  /** Feed one streamed chunk; returns the thinking/answer text it contained. */
  feed(chunk: string): ThinkSplit
  /** Flush any held partial-tag carry at end of stream. */
  flush(): ThinkSplit
}

export function createThinkParser(): ThinkParser {
  let inside = false
  let carry = ''

  function feed(chunk: string): ThinkSplit {
    const s = carry + chunk
    carry = ''
    let thinking = ''
    let answer = ''
    let i = 0
    while (i < s.length) {
      const tag = inside ? CLOSE : OPEN
      const idx = s.indexOf(tag, i)
      if (idx === -1) {
        const rest = s.slice(i)
        const hold = partialTailLen(rest, tag)
        const emit = hold ? rest.slice(0, rest.length - hold) : rest
        if (inside) thinking += emit
        else answer += emit
        carry = hold ? rest.slice(rest.length - hold) : ''
        break
      }
      const emit = s.slice(i, idx)
      if (inside) thinking += emit
      else answer += emit
      inside = !inside
      i = idx + tag.length
    }
    return { thinking, answer }
  }

  function flush(): ThinkSplit {
    const rest = carry
    carry = ''
    if (!rest) return { thinking: '', answer: '' }
    // A held partial tag that never completed is literal text in its channel.
    return inside ? { thinking: rest, answer: '' } : { thinking: '', answer: rest }
  }

  return { feed, flush }
}

/** One-shot split for fully-buffered text (e.g. a persisted assistant turn). */
export function splitThinking(full: string): ThinkSplit {
  const p = createThinkParser()
  const a = p.feed(full)
  const b = p.flush()
  return { thinking: a.thinking + b.thinking, answer: a.answer + b.answer }
}
