//! Shared Server-Sent Events framing (Requirement 8.1-8.4).
//!
//! Provider adapters feed raw network bytes into an [`SseDecoder`] and get back
//! complete [`SseEvent`]s. The decoder is provider-neutral: it only frames
//! events (handling `\n`/`\r\n`, comment lines, multi-`data:` joining,
//! `event:` names, and optional `id:` values). Interpreting an event's JSON
//! payload is the adapter's job.
//!
//! Byte-level buffering (rather than `from_utf8_lossy` per network chunk) is
//! deliberate: a chunk boundary can fall in the middle of a multi-byte UTF-8
//! sequence. We split on the ASCII `\n` byte — which never appears inside a
//! multi-byte sequence — and only decode complete lines, keeping any trailing
//! partial line buffered for the next push.

/// One framed SSE event. `event` is the optional `event:` field name; `data` is
/// the joined `data:` payload (multiple `data:` lines joined with `\n`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub id: Option<String>,
    pub data: String,
}

/// Incremental SSE framer. Create one per response; call [`push`](Self::push)
/// with each network chunk.
#[derive(Default)]
pub struct SseDecoder {
    buf: Vec<u8>,
    event_name: Option<String>,
    event_id: Option<String>,
    data_lines: Vec<String>,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a network chunk; returns any events completed by it. A blank line
    /// dispatches the accumulated event.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        self.buf.extend_from_slice(bytes);
        let mut events = Vec::new();

        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            // Drain the line including its trailing '\n'.
            let mut line: Vec<u8> = self.buf.drain(..=pos).collect();
            line.pop(); // remove '\n'
            if line.last() == Some(&b'\r') {
                line.pop(); // CRLF -> drop the '\r'
            }
            let line = String::from_utf8_lossy(&line);

            if line.is_empty() {
                // Event boundary: dispatch if we have data buffered.
                if !self.data_lines.is_empty() {
                    events.push(SseEvent {
                        event: self.event_name.take(),
                        id: self.event_id.take(),
                        data: self.data_lines.join("\n"),
                    });
                    self.data_lines.clear();
                }
                self.event_name = None;
                self.event_id = None;
                continue;
            }

            if line.starts_with(':') {
                // Comment / keepalive line — ignore.
                continue;
            }

            if let Some(rest) = line.strip_prefix("data:") {
                // Per spec, a single leading space after the colon is stripped.
                self.data_lines
                    .push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
            } else if let Some(rest) = line.strip_prefix("event:") {
                self.event_name = Some(rest.strip_prefix(' ').unwrap_or(rest).trim().to_string());
            } else if let Some(rest) = line.strip_prefix("id:") {
                self.event_id = Some(rest.strip_prefix(' ').unwrap_or(rest).to_string());
            }
            // `retry:` and unknown fields are intentionally ignored.
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_lf_separated_data_events() {
        let mut d = SseDecoder::new();
        let evs = d.push(b"data: hello\n\ndata: world\n\n");
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0].data, "hello");
        assert_eq!(evs[1].data, "world");
    }

    #[test]
    fn handles_crlf_and_event_names() {
        let mut d = SseDecoder::new();
        let evs = d.push(b"event: content_block_delta\r\ndata: {\"x\":1}\r\n\r\n");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].event.as_deref(), Some("content_block_delta"));
        assert_eq!(evs[0].id, None);
        assert_eq!(evs[0].data, "{\"x\":1}");
    }

    #[test]
    fn captures_id_and_done_payload() {
        let mut d = SseDecoder::new();
        let evs = d.push(b"id: event-42\ndata: [DONE]\n\n");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].id.as_deref(), Some("event-42"));
        assert_eq!(evs[0].event, None);
        assert_eq!(evs[0].data, "[DONE]");
    }

    #[test]
    fn id_without_data_does_not_leak_to_next_event() {
        let mut d = SseDecoder::new();
        assert!(d.push(b"id: stale\n\n").is_empty());
        let evs = d.push(b"data: fresh\n\n");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].id, None);
        assert_eq!(evs[0].data, "fresh");
    }

    #[test]
    fn joins_multiple_data_lines_with_newline() {
        let mut d = SseDecoder::new();
        let evs = d.push(b"data: line1\ndata: line2\n\n");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].data, "line1\nline2");
    }

    #[test]
    fn ignores_comment_and_blank_keepalive_lines() {
        let mut d = SseDecoder::new();
        let evs = d.push(b": keepalive comment\n\n");
        assert!(evs.is_empty());
    }

    #[test]
    fn reassembles_event_split_across_chunks() {
        let mut d = SseDecoder::new();
        assert!(d.push(b"data: hel").is_empty());
        assert!(d.push(b"lo\n").is_empty()); // line complete, event not yet
        let evs = d.push(b"\n");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].data, "hello");
    }

    #[test]
    fn does_not_corrupt_multibyte_split_across_chunks() {
        // "é" is 0xC3 0xA9; split the two bytes across pushes.
        let mut d = SseDecoder::new();
        assert!(d.push(b"data: \xc3").is_empty());
        let evs = d.push(b"\xa9\n\n");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].data, "é");
    }
}
