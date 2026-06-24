//! Line-capped scrollback buffer (Milestone 3, Wave 5 — GWEN-250).
//!
//! PTY output is unbounded: a `cargo build`, `ls -R`, or a runaway print loop
//! would grow a plain `Vec<u8>` without limit and eventually exhaust memory.
//! This ring buffer caps retention by *line count* (the unit users think in,
//! and what the M3 spec mandates): once more than `max_lines` completed lines
//! are held, the oldest are dropped first (FIFO).
//!
//! It is fed raw byte chunks straight off the reader thread — chunks do not
//! align to line boundaries, so a chunk may contain a partial line, several
//! lines, or span the middle of a line. The buffer therefore tracks one
//! *pending* (not-yet-newline-terminated) line and promotes it to a completed
//! line each time a `\n` arrives. Trimming counts completed lines; the pending
//! line is always kept (it's the live, in-progress output).
//!
//! Bytes are stored verbatim, including ANSI escape sequences and the `\n`
//! terminators, so the rendered scrollback is byte-for-byte faithful to what
//! the PTY emitted (minus whatever fell off the front).

use std::collections::VecDeque;

/// Default scrollback cap (lines). Matches the M3 spec's "misal 10.000".
pub const DEFAULT_MAX_LINES: usize = 10_000;

/// A scrollback buffer that retains at most `max_lines` completed lines plus the
/// current pending (unterminated) line.
#[derive(Debug)]
pub struct RingBuffer {
    /// Completed lines, each INCLUDING its trailing `\n`. Front = oldest.
    lines: VecDeque<Vec<u8>>,
    /// The current line being accumulated (no trailing `\n` yet).
    pending: Vec<u8>,
    max_lines: usize,
}

impl RingBuffer {
    /// Creates a buffer capped at `max_lines` completed lines. A `max_lines` of
    /// 0 is treated as 1 so there is always somewhere to put output.
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            pending: Vec::new(),
            max_lines: max_lines.max(1),
        }
    }

    /// Appends a raw output chunk, splitting on `\n` into completed lines and
    /// trimming the oldest once the cap is exceeded.
    pub fn push(&mut self, chunk: &[u8]) {
        for &byte in chunk {
            self.pending.push(byte);
            if byte == b'\n' {
                // Promote the finished line (keeps its '\n') and start fresh.
                let line = std::mem::take(&mut self.pending);
                self.lines.push_back(line);
                self.trim();
            }
        }
    }

    /// Drops oldest completed lines until within `max_lines`.
    fn trim(&mut self) {
        while self.lines.len() > self.max_lines {
            self.lines.pop_front();
        }
    }

    /// Number of completed lines currently retained (excludes the pending line).
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Total bytes currently held (completed lines + pending). Useful for tests
    /// and for asserting bounded memory.
    pub fn byte_len(&self) -> usize {
        self.lines.iter().map(|l| l.len()).sum::<usize>() + self.pending.len()
    }

    /// Returns the full retained scrollback as bytes: every completed line in
    /// order, then the pending line.
    pub fn contents(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.byte_len());
        for line in &self.lines {
            out.extend_from_slice(line);
        }
        out.extend_from_slice(&self.pending);
        out
    }

    /// Retained scrollback decoded UTF-8-lossily.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.contents()).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_is_empty() {
        let rb = RingBuffer::new(10);
        assert_eq!(rb.line_count(), 0);
        assert_eq!(rb.byte_len(), 0);
        assert_eq!(rb.to_string_lossy(), "");
    }

    #[test]
    fn pending_line_is_retained_before_newline() {
        let mut rb = RingBuffer::new(10);
        rb.push(b"partial output no newline yet");
        // No completed line, but the pending bytes are still readable.
        assert_eq!(rb.line_count(), 0);
        assert_eq!(rb.to_string_lossy(), "partial output no newline yet");
    }

    #[test]
    fn chunk_split_across_line_boundary_reassembles() {
        let mut rb = RingBuffer::new(10);
        // A single logical line "hello\n" arriving in three arbitrary chunks.
        rb.push(b"hel");
        rb.push(b"lo");
        rb.push(b"\nworld");
        assert_eq!(rb.line_count(), 1);
        assert_eq!(rb.to_string_lossy(), "hello\nworld");
    }

    #[test]
    fn multiple_lines_in_one_chunk() {
        let mut rb = RingBuffer::new(10);
        rb.push(b"a\nb\nc\n");
        assert_eq!(rb.line_count(), 3);
        assert_eq!(rb.to_string_lossy(), "a\nb\nc\n");
    }

    #[test]
    fn oldest_lines_are_dropped_past_the_cap() {
        let mut rb = RingBuffer::new(3);
        for i in 0..6 {
            rb.push(format!("line{i}\n").as_bytes());
        }
        // Only the last 3 completed lines survive.
        assert_eq!(rb.line_count(), 3);
        assert_eq!(rb.to_string_lossy(), "line3\nline4\nline5\n");
    }

    #[test]
    fn trimming_keeps_the_pending_line() {
        let mut rb = RingBuffer::new(2);
        rb.push(b"l0\nl1\nl2\n"); // 3 completed -> trimmed to last 2
        rb.push(b"in-progress"); // pending, no newline
        assert_eq!(rb.line_count(), 2);
        assert_eq!(rb.to_string_lossy(), "l1\nl2\nin-progress");
    }

    #[test]
    fn max_lines_zero_is_clamped_to_one() {
        let mut rb = RingBuffer::new(0);
        rb.push(b"a\nb\n");
        assert_eq!(rb.line_count(), 1);
        assert_eq!(rb.to_string_lossy(), "b\n");
    }

    // The core GWEN-250 guarantee: under a flood of output, retained memory
    // stays bounded by the cap rather than growing without limit.
    #[test]
    fn memory_stays_bounded_under_flood() {
        let cap = 100;
        let mut rb = RingBuffer::new(cap);
        // 100_000 lines of fixed width pushed in; only `cap` may remain.
        for i in 0..100_000 {
            rb.push(format!("output line number {i}\n").as_bytes());
        }
        assert_eq!(rb.line_count(), cap);
        // Bytes are bounded too: at most cap lines, each a bounded width.
        // Widest line "output line number 99999\n" = 26 bytes; allow slack.
        assert!(
            rb.byte_len() <= cap * 64,
            "byte_len {} exceeded bound",
            rb.byte_len()
        );
        // The newest line must be present, the oldest gone.
        let s = rb.to_string_lossy();
        assert!(s.contains("output line number 99999"));
        assert!(!s.contains("output line number 0\n"));
    }

    #[test]
    fn binary_bytes_are_preserved_verbatim() {
        let mut rb = RingBuffer::new(10);
        // ANSI escape + raw bytes, no newline: stored as-is.
        rb.push(b"\x1b[31mred\x1b[0m");
        assert_eq!(rb.contents(), b"\x1b[31mred\x1b[0m");
    }
}
