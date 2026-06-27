import type { Terminal } from '@xterm/xterm'

/**
 * Terminal output frame-limiter (M19 Wave 4, GWEN-378).
 *
 * XTerm writes are synchronous repaints. Writing every PTY chunk as it arrives
 * (e.g. `cargo build` emitting hundreds of chunks a second) forces a repaint per
 * chunk and starves the editor's frame budget. This scheduler coalesces all
 * chunks that arrive within one animation frame into a single `term.write`, so
 * heavy output costs at most one repaint per frame (~60fps).
 *
 * It is also the pause point: while paused (terminal tab hidden, or window in
 * the background) chunks accumulate in a bounded ring buffer and NO writes
 * happen, so a backgrounded `cargo build` never repaints. `resume()` flushes
 * the buffered tail in one write.
 *
 * One instance per terminal session (each owns its own buffer), unlike a module
 * singleton which couldn't serve multiple concurrent sessions.
 */

/** Cap on buffered bytes while paused — the most recent ~1MB is kept, matching
 *  XTerm's own scrollback drop behavior for older lines. */
const MAX_BUFFERED_BYTES = 1 << 20

export class TerminalScheduler {
  private term: Terminal
  private buffer: Uint8Array[] = []
  private bufferedBytes = 0
  private rafId: number | null = null
  private paused = false
  private disposed = false

  constructor(term: Terminal) {
    this.term = term
  }

  /** Queue a chunk. When running, schedules a single flush on the next frame;
   *  when paused, it just accumulates (bounded) until `resume()`. */
  write(bytes: Uint8Array): void {
    if (this.disposed) return
    this.push(bytes)
    if (!this.paused) this.schedule()
  }

  private push(bytes: Uint8Array): void {
    this.buffer.push(bytes)
    this.bufferedBytes += bytes.length
    // Keep only the most recent ~1MB so a runaway/paused stream stays bounded.
    while (this.bufferedBytes > MAX_BUFFERED_BYTES && this.buffer.length > 1) {
      this.bufferedBytes -= this.buffer.shift()!.length
    }
  }

  private schedule(): void {
    if (this.rafId !== null) return
    this.rafId = requestAnimationFrame(() => {
      this.rafId = null
      this.flush()
    })
  }

  /** Write all buffered chunks as one batch (one repaint). */
  private flush(): void {
    if (this.disposed || this.buffer.length === 0) return
    const batch = this.buffer
    this.buffer = []
    this.bufferedBytes = 0
    // Concatenate into a single Uint8Array so XTerm sees one write call.
    const merged = mergeChunks(batch)
    this.term.write(merged)
  }

  /** Stop writing (tab hidden / window backgrounded). Chunks keep buffering. */
  pause(): void {
    this.paused = true
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId)
      this.rafId = null
    }
  }

  /** Resume writing and flush whatever buffered while paused, in one repaint. */
  resume(): void {
    if (!this.paused) return
    this.paused = false
    this.flush()
  }

  /** Drop pending output and cancel any scheduled frame (teardown). */
  dispose(): void {
    this.disposed = true
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId)
      this.rafId = null
    }
    this.buffer = []
    this.bufferedBytes = 0
  }
}

/** Concatenate chunks into one buffer. Exported for unit testing. */
export function mergeChunks(chunks: Uint8Array[]): Uint8Array {
  if (chunks.length === 1) return chunks[0]
  let total = 0
  for (const c of chunks) total += c.length
  const out = new Uint8Array(total)
  let offset = 0
  for (const c of chunks) {
    out.set(c, offset)
    offset += c.length
  }
  return out
}
