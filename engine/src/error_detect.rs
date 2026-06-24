//! Lightweight terminal error detection (Milestone 3, Wave 6 — GWEN-251).
//!
//! Scans PTY output *reactively, per chunk* for common error signatures and
//! raises a "possible error" signal. This is deliberately NOT a periodic full
//! re-scan of the whole scrollback (which would waste CPU on every update);
//! instead the detector keeps a one-line carry-over and only inspects newly
//! completed lines as they arrive off the reader thread.
//!
//! It is intentionally dependency-free (no `regex` crate): the signatures are
//! fixed substrings, matched against an ANSI-stripped copy of each line so that
//! colourised output like `\x1b[31merror:\x1b[0m` still matches. The result is a
//! best-effort heuristic meant to *flag* output for a future AI "explain this
//! error" feature — it is not a compiler and will both miss and over-match. The
//! curated pattern set below favours low false positives.

/// One curated error signature. `needle` is matched as a substring of the
/// ANSI-stripped line; `case_insensitive` lowercases both sides first.
struct Pattern {
    needle: &'static str,
    case_insensitive: bool,
    /// Human-facing label for the matched class (sent to the frontend).
    label: &'static str,
}

/// Curated common error signatures. Tuned for low false positives; broad bare
/// words like "failed" are deliberately excluded.
const PATTERNS: &[Pattern] = &[
    // Generic compiler / tool diagnostics.
    Pattern {
        needle: "error:",
        case_insensitive: true,
        label: "error",
    },
    Pattern {
        needle: "error[",
        case_insensitive: true,
        label: "rustc-error-code",
    },
    Pattern {
        needle: "fatal:",
        case_insensitive: true,
        label: "fatal",
    },
    Pattern {
        needle: "fatal error",
        case_insensitive: true,
        label: "fatal",
    },
    Pattern {
        needle: "could not compile",
        case_insensitive: true,
        label: "compile-failed",
    },
    // Language runtimes / stack traces.
    Pattern {
        needle: "panicked at",
        case_insensitive: true,
        label: "rust-panic",
    },
    Pattern {
        needle: "Traceback (most recent call last)",
        case_insensitive: false,
        label: "python-traceback",
    },
    Pattern {
        needle: "Exception",
        case_insensitive: false,
        label: "exception",
    },
    // Shell / tool failures.
    Pattern {
        needle: "command not found",
        case_insensitive: true,
        label: "command-not-found",
    },
    Pattern {
        needle: "is not recognized",
        case_insensitive: true,
        label: "command-not-found",
    },
    Pattern {
        needle: "no such file or directory",
        case_insensitive: true,
        label: "no-such-file",
    },
    Pattern {
        needle: "npm ERR!",
        case_insensitive: false,
        label: "npm-error",
    },
];

/// Hard cap on the carry-over line so a stream that never emits a newline can't
/// grow `pending` without bound. Beyond this we treat what we have as a line.
const MAX_PENDING: usize = 64 * 1024;

/// A detected error occurrence: the (ANSI-stripped, trimmed) line and the label
/// of the signature it matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorSignal {
    pub line: String,
    pub label: String,
}

/// Stateful, reactive error detector. Feed it chunks; it returns any signals
/// found in newly-completed lines.
#[derive(Debug, Default)]
pub struct ErrorDetector {
    /// Carry-over bytes of the current not-yet-newline-terminated line.
    pending: Vec<u8>,
}

impl ErrorDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one output chunk. Splits on `\n`, scans each completed line, and
    /// returns the signals found in *this* chunk (only new lines are scanned —
    /// the whole buffer is never re-read).
    pub fn scan_chunk(&mut self, chunk: &[u8]) -> Vec<ErrorSignal> {
        let mut signals = Vec::new();
        for &byte in chunk {
            if byte == b'\n' {
                let line = std::mem::take(&mut self.pending);
                if let Some(sig) = match_line(&line) {
                    signals.push(sig);
                }
            } else {
                self.pending.push(byte);
                if self.pending.len() >= MAX_PENDING {
                    // Pathological no-newline stream: scan and reset so we don't
                    // grow unbounded or stall detection.
                    let line = std::mem::take(&mut self.pending);
                    if let Some(sig) = match_line(&line) {
                        signals.push(sig);
                    }
                }
            }
        }
        signals
    }

    /// Scans any trailing partial line (e.g. a process that exits without a
    /// final newline). Call once at end-of-stream if needed.
    pub fn flush(&mut self) -> Option<ErrorSignal> {
        if self.pending.is_empty() {
            return None;
        }
        let line = std::mem::take(&mut self.pending);
        match_line(&line)
    }
}

/// Strips ANSI escape sequences (CSI `ESC[…<final>` and OSC `ESC]…BEL/ST`) from
/// `bytes`, returning the visible text lossily decoded as UTF-8. Shared with the
/// dev-server detector (M5), which needs the same colour-stripping to match URLs.
pub(crate) fn strip_ansi(bytes: &[u8]) -> String {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'[' => {
                    // CSI: skip until a final byte in 0x40..=0x7E.
                    i += 2;
                    while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                        i += 1;
                    }
                    i += 1; // consume the final byte
                    continue;
                }
                b']' => {
                    // OSC: skip until BEL (0x07) or ST (ESC \).
                    i += 2;
                    while i < bytes.len() && bytes[i] != 0x07 {
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                    i += 1;
                    continue;
                }
                _ => {
                    // Other escape (e.g. ESC + single char): skip both.
                    i += 2;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Returns the first matching signal for `line`, or None.
fn match_line(line: &[u8]) -> Option<ErrorSignal> {
    let clean = strip_ansi(line);
    let trimmed = clean.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    for p in PATTERNS {
        let hit = if p.case_insensitive {
            lower.contains(&p.needle.to_lowercase())
        } else {
            trimmed.contains(p.needle)
        };
        if hit {
            return Some(ErrorSignal {
                line: trimmed.to_string(),
                label: p.label.to_string(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(sigs: &[ErrorSignal]) -> Vec<&str> {
        sigs.iter().map(|s| s.label.as_str()).collect()
    }

    #[test]
    fn detects_generic_error_line() {
        let mut d = ErrorDetector::new();
        let sigs = d.scan_chunk(b"error: something went wrong\n");
        assert_eq!(labels(&sigs), vec!["error"]);
        assert_eq!(sigs[0].line, "error: something went wrong");
    }

    #[test]
    fn detects_rust_error_code_and_panic() {
        let mut d = ErrorDetector::new();
        let a = d.scan_chunk(b"error[E0382]: borrow of moved value\n");
        assert_eq!(labels(&a), vec!["rustc-error-code"]);
        let b = d.scan_chunk(b"thread 'main' panicked at src/main.rs:4:5\n");
        assert_eq!(labels(&b), vec!["rust-panic"]);
    }

    #[test]
    fn detects_through_ansi_colour_codes() {
        let mut d = ErrorDetector::new();
        // Colourised "error:" as cargo/gcc emit it.
        let sigs = d.scan_chunk(b"\x1b[1m\x1b[31merror:\x1b[0m mismatched types\n");
        assert_eq!(labels(&sigs), vec!["error"]);
        // The stored line is ANSI-stripped.
        assert_eq!(sigs[0].line, "error: mismatched types");
    }

    #[test]
    fn detects_across_chunk_boundary() {
        let mut d = ErrorDetector::new();
        // "error:" split mid-word across two chunks, newline in a third.
        assert!(d.scan_chunk(b"err").is_empty());
        assert!(d.scan_chunk(b"or: boom").is_empty());
        let sigs = d.scan_chunk(b"\n");
        assert_eq!(labels(&sigs), vec!["error"]);
    }

    #[test]
    fn does_not_match_only_on_partial_line_until_newline() {
        let mut d = ErrorDetector::new();
        // Reactive: a complete error needs its newline; no double-count.
        assert!(d.scan_chunk(b"error: pending").is_empty());
        let sigs = d.scan_chunk(b"\n");
        assert_eq!(sigs.len(), 1);
    }

    #[test]
    fn clean_output_does_not_false_positive() {
        let mut d = ErrorDetector::new();
        let sigs =
            d.scan_chunk(b"Compiling foo v0.1.0\n  Finished in 2s\n0 errors, 0 warnings\nDone.\n");
        assert!(sigs.is_empty(), "clean output flagged: {:?}", sigs);
    }

    #[test]
    fn detects_shell_command_not_found_both_platforms() {
        let mut d = ErrorDetector::new();
        let sh = d.scan_chunk(b"bash: foo: command not found\n");
        assert_eq!(labels(&sh), vec!["command-not-found"]);
        let win = d.scan_chunk(b"'foo' is not recognized as an internal or external command,\n");
        assert_eq!(labels(&win), vec!["command-not-found"]);
    }

    #[test]
    fn detects_python_traceback_and_npm_error() {
        let mut d = ErrorDetector::new();
        let py = d.scan_chunk(b"Traceback (most recent call last):\n");
        assert_eq!(labels(&py), vec!["python-traceback"]);
        let npm = d.scan_chunk(b"npm ERR! code ELIFECYCLE\n");
        assert_eq!(labels(&npm), vec!["npm-error"]);
    }

    #[test]
    fn flush_scans_trailing_unterminated_line() {
        let mut d = ErrorDetector::new();
        assert!(d.scan_chunk(b"fatal: not a git repository").is_empty());
        let sig = d.flush().expect("trailing line should match");
        assert_eq!(sig.label, "fatal");
    }

    #[test]
    fn multiple_errors_in_one_chunk() {
        let mut d = ErrorDetector::new();
        let sigs = d.scan_chunk(b"error: first\nok line\nerror: second\n");
        assert_eq!(labels(&sigs), vec!["error", "error"]);
    }
}
