//! Dev-server "ready" detection (Milestone 5 — Web Preview).
//!
//! Reactively scans PTY output, *per chunk*, for the line a local dev server
//! prints once it has bound its port — e.g. Vite's `➜  Local: http://localhost:5173/`,
//! Next.js's `- Local: http://localhost:3000`, CRA's `Local: http://localhost:3000`.
//! When such a line appears the IDE can auto-open a web preview pointed at that
//! URL (M5) with no user action — the primary detection path, ahead of the
//! port-poll fallback.
//!
//! Shape and philosophy deliberately mirror [`crate::error_detect`]:
//!
//!   * **Reactive & stateful** — a one-line carry-over; only newly completed
//!     lines are inspected, never a re-scan of the whole scrollback.
//!   * **Dependency-free** — no `regex` crate. The framework-specific patterns in
//!     the M5 spec (Vite/Next/CRA) all reduce to "a `localhost` / `127.0.0.1`
//!     URL with an explicit port", so a single hand-rolled host:port extractor
//!     subsumes them. ANSI escapes are stripped first (reusing
//!     [`crate::error_detect::strip_ansi`]) so colourised URLs still match.
//!   * **Best-effort heuristic** — it can over-match (any `http://localhost:PORT`
//!     echoed to the terminal will trip it). To bound the blast radius the
//!     detector *latches*: it reports the first ready URL only, then stays quiet
//!     until [`DevServerDetector::reset`]. That suits the use case (auto-open the
//!     preview once); a server that re-announces after a restart does not reopen
//!     a pane the user may have closed.

use crate::error_detect::strip_ansi;

/// Hard cap on the carry-over line so a stream that never emits a newline can't
/// grow `pending` without bound. Matches [`crate::error_detect`]'s cap.
const MAX_PENDING: usize = 64 * 1024;

/// Loopback host markers we recognise in a ready line. A `0.0.0.0` bind address
/// is matched here but normalised to `localhost` in the emitted URL (browsers
/// cannot navigate to `0.0.0.0`, notably on Windows). Order is irrelevant: the
/// earliest match position in the line wins.
const HOSTS: &[&[u8]] = &[b"localhost", b"127.0.0.1", b"0.0.0.0"];

/// A detected dev server, ready to preview. `url` is a browsable base URL (no
/// trailing path), `port` the bound port parsed from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevServerSignal {
    /// Browsable base URL, e.g. `http://localhost:5173`. The scheme is taken from
    /// the line when explicit (`https`), else defaults to `http`; a `0.0.0.0`
    /// bind address is normalised to `localhost`.
    pub url: String,
    /// The bound port (1..=65535).
    pub port: u16,
}

/// Stateful, reactive, *latching* dev-server-ready detector. Feed it chunks; it
/// returns the first ready URL it sees and then nothing more until [`reset`].
///
/// [`reset`]: DevServerDetector::reset
#[derive(Debug, Default)]
pub struct DevServerDetector {
    /// Carry-over bytes of the current not-yet-newline-terminated line.
    pending: Vec<u8>,
    /// Latch: once a ready line has been reported, further scanning is skipped.
    found: bool,
}

impl DevServerDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one output chunk. Splits on `\n`, scans each completed line, and
    /// returns the first ready URL found (across this and prior chunks). Once a
    /// URL has been returned the detector latches and every later call yields
    /// `None` until [`reset`](Self::reset).
    pub fn scan_chunk(&mut self, chunk: &[u8]) -> Option<DevServerSignal> {
        if self.found {
            return None;
        }
        for &byte in chunk {
            if byte == b'\n' {
                let line = std::mem::take(&mut self.pending);
                if let Some(sig) = match_ready_line(&line) {
                    self.found = true;
                    return Some(sig);
                }
            } else {
                self.pending.push(byte);
                if self.pending.len() >= MAX_PENDING {
                    // Pathological no-newline stream: scan and reset the carry-
                    // over so we neither grow unbounded nor stall detection.
                    let line = std::mem::take(&mut self.pending);
                    if let Some(sig) = match_ready_line(&line) {
                        self.found = true;
                        return Some(sig);
                    }
                }
            }
        }
        None
    }

    /// Scans any trailing partial line (e.g. a server that prints its ready line
    /// without a trailing newline before output pauses). Call once if needed.
    pub fn flush(&mut self) -> Option<DevServerSignal> {
        if self.found || self.pending.is_empty() {
            return None;
        }
        let line = std::mem::take(&mut self.pending);
        let sig = match_ready_line(&line);
        if sig.is_some() {
            self.found = true;
        }
        sig
    }

    /// Whether a ready URL has been detected (and the detector is latched).
    pub fn detected(&self) -> bool {
        self.found
    }

    /// Clear the latch and carry-over so the next ready line is reported again
    /// (e.g. to re-arm after the user closes the preview).
    pub fn reset(&mut self) {
        self.pending.clear();
        self.found = false;
    }
}

/// Returns the dev-server signal for the first loopback `host:port` on `line`,
/// or `None`. The line is ANSI-stripped first so colourised URLs match.
fn match_ready_line(line: &[u8]) -> Option<DevServerSignal> {
    let clean = strip_ansi(line);
    let bytes = clean.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        for host in HOSTS {
            if ci_match_at(bytes, i, host) {
                if let Some(port) = parse_port_after_colon(bytes, i + host.len()) {
                    // `0.0.0.0` is not browsable; show it as localhost. Other
                    // markers are emitted as their canonical (lowercase) form.
                    let host_str = if *host == b"0.0.0.0" {
                        "localhost"
                    } else {
                        // Safe: HOSTS entries are ASCII.
                        std::str::from_utf8(host).unwrap()
                    };
                    let scheme = scheme_before(bytes, i);
                    return Some(DevServerSignal {
                        url: format!("{scheme}://{host_str}:{port}"),
                        port,
                    });
                }
            }
        }
        i += 1;
    }
    None
}

/// True if `needle` (ASCII) matches `haystack` at `pos`, case-insensitively.
fn ci_match_at(haystack: &[u8], pos: usize, needle: &[u8]) -> bool {
    haystack.get(pos..pos + needle.len()).is_some_and(|slice| {
        slice
            .iter()
            .zip(needle)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

/// Parses a port immediately following a host: expects `:` at `at`, then 1..=5
/// digits forming a value in 1..=65535. Returns `None` otherwise (no colon, no
/// digits, too many digits, or out of range — so `localhost` with no port, or
/// `localhost:123456`, never matches).
fn parse_port_after_colon(bytes: &[u8], at: usize) -> Option<u16> {
    if bytes.get(at) != Some(&b':') {
        return None;
    }
    let start = at + 1;
    let mut end = start;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let len = end - start;
    if len == 0 || len > 5 {
        return None;
    }
    // Safe: ASCII digits only.
    let port: u32 = std::str::from_utf8(&bytes[start..end])
        .unwrap()
        .parse()
        .ok()?;
    if (1..=65535).contains(&port) {
        Some(port as u16)
    } else {
        None
    }
}

/// Returns the URL scheme for a host starting at `host_start`: `"https"` when the
/// bytes immediately before it are `https://`, else `"http"` (covering both an
/// explicit `http://` and a bare `localhost:PORT` with no scheme).
fn scheme_before(bytes: &[u8], host_start: usize) -> &'static str {
    let prefixed_by = |scheme: &[u8]| {
        host_start
            .checked_sub(scheme.len())
            .is_some_and(|p| ci_match_at(bytes, p, scheme))
    };
    if prefixed_by(b"https://") {
        "https"
    } else {
        "http"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(input: &[u8]) -> Option<DevServerSignal> {
        DevServerDetector::new().scan_chunk(input)
    }

    #[test]
    fn detects_vite_local_line() {
        // Vite prints an arrow glyph + "Local:" + a trailing slash.
        let sig = scan(b"  Local:   http://localhost:5173/\n").expect("vite line should match");
        assert_eq!(sig.url, "http://localhost:5173");
        assert_eq!(sig.port, 5173);
    }

    #[test]
    fn detects_nextjs_and_cra_local_lines() {
        let next = scan(b"- Local:        http://localhost:3000\n").expect("next line");
        assert_eq!(next.url, "http://localhost:3000");
        assert_eq!(next.port, 3000);

        let cra = scan(b"  Local:            http://localhost:3001\n").expect("cra line");
        assert_eq!(cra.port, 3001);
    }

    #[test]
    fn detects_127_0_0_1_generic_line() {
        let sig = scan(b"Server running at http://127.0.0.1:8080/\n").expect("generic line");
        assert_eq!(sig.url, "http://127.0.0.1:8080");
        assert_eq!(sig.port, 8080);
    }

    #[test]
    fn detects_bare_host_port_without_scheme() {
        // No scheme present: default to http.
        let sig = scan(b"listening on localhost:4200\n").expect("bare host:port");
        assert_eq!(sig.url, "http://localhost:4200");
        assert_eq!(sig.port, 4200);
    }

    #[test]
    fn preserves_https_scheme() {
        let sig = scan(b"  Local: https://localhost:5173/\n").expect("https line");
        assert_eq!(sig.url, "https://localhost:5173");
    }

    #[test]
    fn normalises_0_0_0_0_to_localhost() {
        let sig = scan(b"listening on http://0.0.0.0:8000\n").expect("0.0.0.0 line");
        assert_eq!(sig.url, "http://localhost:8000");
        assert_eq!(sig.port, 8000);
    }

    #[test]
    fn detects_through_ansi_colour_codes() {
        // Vite colourises the URL; detection runs on the ANSI-stripped text.
        let sig = scan(b"  \x1b[32mLocal:\x1b[0m \x1b[36mhttp://localhost:5173/\x1b[0m\n")
            .expect("colourised line should match");
        assert_eq!(sig.url, "http://localhost:5173");
    }

    #[test]
    fn detects_across_chunk_boundary() {
        let mut d = DevServerDetector::new();
        assert!(d.scan_chunk(b"  Local:  http://local").is_none());
        assert!(d.scan_chunk(b"host:517").is_none());
        let sig = d.scan_chunk(b"3/\n").expect("completed across chunks");
        assert_eq!(sig.port, 5173);
    }

    #[test]
    fn requires_newline_before_matching() {
        let mut d = DevServerDetector::new();
        // The line is complete content-wise but unterminated: not matched yet.
        assert!(d.scan_chunk(b"  Local: http://localhost:5173/").is_none());
        let sig = d.scan_chunk(b"\n").expect("matches once newline arrives");
        assert_eq!(sig.port, 5173);
    }

    #[test]
    fn latches_on_first_ready_url() {
        let mut d = DevServerDetector::new();
        let first = d
            .scan_chunk(b"  Local: http://localhost:5173/\n")
            .expect("first");
        assert_eq!(first.port, 5173);
        assert!(d.detected());
        // A later, different localhost URL is ignored while latched.
        assert!(
            d.scan_chunk(b"  Network: http://localhost:5174/\n")
                .is_none()
        );
    }

    #[test]
    fn reset_re_arms_detection() {
        let mut d = DevServerDetector::new();
        d.scan_chunk(b"http://localhost:5173/\n").expect("first");
        d.reset();
        assert!(!d.detected());
        let again = d
            .scan_chunk(b"http://localhost:3000/\n")
            .expect("after reset");
        assert_eq!(again.port, 3000);
    }

    #[test]
    fn picks_earliest_host_on_a_line() {
        // Both Local and Network on one line: the first (Local) wins.
        let sig = scan(b"Local: http://localhost:5173/ Network: http://127.0.0.1:5173/\n")
            .expect("line with two urls");
        assert_eq!(sig.url, "http://localhost:5173");
    }

    #[test]
    fn ignores_localhost_without_a_port() {
        // A bare host mention (no `:port`) must not match.
        assert!(scan(b"could not connect to localhost\n").is_none());
        assert!(scan(b"see the localhost: section of the docs\n").is_none());
    }

    #[test]
    fn rejects_out_of_range_and_overlong_ports() {
        assert!(scan(b"http://localhost:0\n").is_none(), "port 0 is invalid");
        assert!(
            scan(b"http://localhost:99999\n").is_none(),
            "port > 65535 invalid"
        );
        assert!(
            scan(b"http://localhost:123456\n").is_none(),
            "6 digits is not a port"
        );
    }

    #[test]
    fn clean_output_does_not_false_positive() {
        assert!(scan(b"Compiling app v0.1.0\n  Finished in 2s\nDone.\n").is_none());
    }

    #[test]
    fn flush_scans_trailing_unterminated_ready_line() {
        let mut d = DevServerDetector::new();
        assert!(d.scan_chunk(b"  Local: http://localhost:5173/").is_none());
        let sig = d.flush().expect("trailing line should match");
        assert_eq!(sig.port, 5173);
        // Latched after a flush hit, too.
        assert!(d.detected());
    }
}
