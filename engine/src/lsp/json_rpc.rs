//! JSON-RPC 2.0 framing and request tracking for LSP over stdio (Milestone 6).
//!
//! Wire format (Requirement 7.1):
//! ```text
//! Content-Length: <byte_count>\r\n
//! \r\n
//! <utf-8 json payload>
//! ```
//!
//! This module is transport-agnostic: it encodes/decodes frames and classifies
//! incoming messages, but never touches a process or socket. `Content-Length`
//! is counted in **bytes**, not characters. [`FrameDecoder`] tolerates partial
//! frames split across reads and multiple frames in one read.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::LspError;

/// JSON-RPC error object carried in a response (`{ code, message, data? }`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// A decoded inbound JSON-RPC message, classified by shape.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// Response to a client request we issued (correlated by `id`).
    Response {
        id: u64,
        result: Option<Value>,
        error: Option<ResponseError>,
    },
    /// Server-initiated notification (`publishDiagnostics`, `logMessage`, …).
    Notification { method: String, params: Value },
    /// Server-initiated request (has an id; M6 replies generically or ignores).
    ServerRequest {
        id: Value,
        method: String,
        params: Value,
    },
    /// A response whose id was not a u64 we recognize. Kept non-fatal.
    UnknownResponse,
}

/// Loose shape used only for classification during decode.
#[derive(Deserialize)]
struct RawMessage {
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<ResponseError>,
}

/// Encode a JSON payload string as a `Content-Length`-framed byte buffer.
pub fn encode_frame(payload: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 32);
    out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", payload.len()).as_bytes());
    out.extend_from_slice(payload.as_bytes());
    out
}

/// Encode a client request frame.
pub fn encode_request(id: u64, method: &str, params: Value) -> Vec<u8> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    encode_frame(&payload.to_string())
}

/// Encode a client notification frame (no id, no response expected).
pub fn encode_notification(method: &str, params: Value) -> Vec<u8> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    });
    encode_frame(&payload.to_string())
}

/// Encode a response to a server-initiated request.
pub fn encode_response(id: Value, result: Value) -> Vec<u8> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    });
    encode_frame(&payload.to_string())
}

/// Classify a single decoded JSON payload into an [`Incoming`].
pub fn parse_message(bytes: &[u8]) -> Result<Incoming, LspError> {
    let raw: RawMessage = serde_json::from_slice(bytes)
        .map_err(|e| LspError::protocol(format!("invalid json-rpc payload: {e}")))?;

    match raw.method {
        Some(method) => {
            let params = raw.params.unwrap_or(Value::Null);
            if let Some(id) = raw.id {
                Ok(Incoming::ServerRequest { id, method, params })
            } else {
                Ok(Incoming::Notification { method, params })
            }
        }
        None => match raw.id.as_ref().and_then(Value::as_u64) {
            Some(id) => Ok(Incoming::Response {
                id,
                result: raw.result,
                error: raw.error,
            }),
            None => Ok(Incoming::UnknownResponse),
        },
    }
}

/// Incremental `Content-Length` frame decoder. Feed bytes with [`FrameDecoder::push`]
/// then drain complete frames with [`FrameDecoder::next_frame`].
#[derive(Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Append freshly-read bytes to the internal buffer.
    pub fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Pop the next complete frame's JSON body, or `Ok(None)` if more bytes are
    /// needed. Malformed headers yield `Err(LspError::Protocol)`.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, LspError> {
        // Find the header/body separator.
        let Some(header_end) = find_subslice(&self.buf, b"\r\n\r\n") else {
            return Ok(None);
        };

        let header_str = std::str::from_utf8(&self.buf[..header_end])
            .map_err(|_| LspError::protocol("non-utf8 frame header"))?;

        let mut content_length: Option<usize> = None;
        for line in header_str.split("\r\n") {
            if let Some(rest) = line
                .strip_prefix("Content-Length:")
                .or_else(|| line.strip_prefix("content-length:"))
            {
                content_length = rest.trim().parse::<usize>().ok();
            }
            // Other headers (e.g. Content-Type) are accepted and ignored.
        }

        let Some(len) = content_length else {
            return Err(LspError::protocol("frame missing Content-Length header"));
        };

        let body_start = header_end + 4; // skip "\r\n\r\n"
        if self.buf.len() < body_start + len {
            return Ok(None); // body not fully arrived yet
        }

        let body = self.buf[body_start..body_start + len].to_vec();
        // Drain the consumed frame, retaining any trailing bytes (next frame).
        self.buf.drain(..body_start + len);
        Ok(Some(body))
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Monotonic `u64` request id generator (Requirement 7.3). Starts at 1.
#[derive(Debug)]
pub struct RequestIdGen {
    next: AtomicU64,
}

impl Default for RequestIdGen {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestIdGen {
    pub fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    pub fn next_id(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
}

/// Map of in-flight client requests by id (Requirement 7.4). Generic over the
/// responder `T` so the transport chooses the channel type while this stays
/// unit-testable. [`PendingRequests::drain`] is the crash-cleanup API.
pub struct PendingRequests<T> {
    map: HashMap<u64, T>,
}

impl<T> Default for PendingRequests<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> PendingRequests<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, id: u64, responder: T) {
        self.map.insert(id, responder);
    }

    /// Remove and return the responder for `id` when a response arrives.
    pub fn take(&mut self, id: u64) -> Option<T> {
        self.map.remove(&id)
    }

    /// Remove and return every pending responder (crash/shutdown cleanup).
    pub fn drain(&mut self) -> Vec<T> {
        self.map.drain().map(|(_, v)| v).collect()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_frame_uses_byte_length() {
        // "é" is two UTF-8 bytes — the header must count bytes, not chars.
        let frame = encode_frame("é");
        let text = String::from_utf8(frame).unwrap();
        assert!(text.starts_with("Content-Length: 2\r\n\r\n"));
        assert!(text.ends_with("é"));
    }

    #[test]
    fn encode_then_decode_round_trip() {
        let frame = encode_request(7, "initialize", serde_json::json!({"k": "v"}));
        let mut dec = FrameDecoder::new();
        dec.push(&frame);
        let body = dec.next_frame().unwrap().unwrap();
        let msg = parse_message(&body).unwrap();
        match msg {
            Incoming::ServerRequest { id, method, .. } => {
                // A request *with* an id round-trips as a ServerRequest shape
                // when parsed in isolation (it has both id and method).
                assert_eq!(method, "initialize");
                assert_eq!(id, Value::from(7u64));
            }
            other => panic!("unexpected: {other:?}"),
        }
        assert!(dec.next_frame().unwrap().is_none());
    }

    #[test]
    fn partial_frame_then_completion() {
        let frame = encode_frame(r#"{"jsonrpc":"2.0","method":"x","params":{}}"#);
        let split = frame.len() / 2;
        let mut dec = FrameDecoder::new();
        dec.push(&frame[..split]);
        assert!(dec.next_frame().unwrap().is_none(), "incomplete -> None");
        dec.push(&frame[split..]);
        let body = dec.next_frame().unwrap().unwrap();
        let msg = parse_message(&body).unwrap();
        assert!(matches!(msg, Incoming::Notification { .. }));
    }

    #[test]
    fn multiple_frames_in_one_stream() {
        let mut stream = Vec::new();
        stream.extend(encode_frame(r#"{"jsonrpc":"2.0","method":"a","params":1}"#));
        stream.extend(encode_frame(r#"{"jsonrpc":"2.0","method":"b","params":2}"#));
        stream.extend(encode_frame(r#"{"jsonrpc":"2.0","id":5,"result":true}"#));

        let mut dec = FrameDecoder::new();
        dec.push(&stream);

        let m1 = parse_message(&dec.next_frame().unwrap().unwrap()).unwrap();
        let m2 = parse_message(&dec.next_frame().unwrap().unwrap()).unwrap();
        let m3 = parse_message(&dec.next_frame().unwrap().unwrap()).unwrap();
        assert!(dec.next_frame().unwrap().is_none());

        assert!(matches!(m1, Incoming::Notification { ref method, .. } if method == "a"));
        assert!(matches!(m2, Incoming::Notification { ref method, .. } if method == "b"));
        match m3 {
            Incoming::Response { id, result, error } => {
                assert_eq!(id, 5);
                assert_eq!(result, Some(Value::Bool(true)));
                assert!(error.is_none());
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn classifies_notification_vs_response_vs_server_request() {
        let n = parse_message(
            br#"{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{}}"#,
        )
        .unwrap();
        assert!(
            matches!(n, Incoming::Notification { ref method, .. } if method.ends_with("publishDiagnostics"))
        );

        let r =
            parse_message(br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"boom"}}"#)
                .unwrap();
        match r {
            Incoming::Response { id, error, .. } => {
                assert_eq!(id, 1);
                assert_eq!(error.unwrap().message, "boom");
            }
            other => panic!("unexpected: {other:?}"),
        }

        let sr = parse_message(
            br#"{"jsonrpc":"2.0","id":99,"method":"workspace/configuration","params":[]}"#,
        )
        .unwrap();
        assert!(
            matches!(sr, Incoming::ServerRequest { ref method, .. } if method == "workspace/configuration")
        );
    }

    #[test]
    fn malformed_json_is_protocol_error_not_panic() {
        let err = parse_message(b"not json").unwrap_err();
        assert!(matches!(err, LspError::Protocol(_)));
    }

    #[test]
    fn missing_content_length_is_protocol_error() {
        let mut dec = FrameDecoder::new();
        dec.push(b"X-Header: 1\r\n\r\nbody");
        let err = dec.next_frame().unwrap_err();
        assert!(matches!(err, LspError::Protocol(_)));
    }

    #[test]
    fn request_ids_are_monotonic() {
        let idgen = RequestIdGen::new();
        let a = idgen.next_id();
        let b = idgen.next_id();
        let c = idgen.next_id();
        assert_eq!((a, b, c), (1, 2, 3));
    }

    #[test]
    fn pending_requests_insert_take_drain() {
        let mut pending: PendingRequests<String> = PendingRequests::new();
        pending.insert(1, "one".into());
        pending.insert(2, "two".into());
        assert_eq!(pending.len(), 2);

        assert_eq!(pending.take(1), Some("one".to_string()));
        assert_eq!(pending.take(1), None);
        assert_eq!(pending.len(), 1);

        let drained = pending.drain();
        assert_eq!(drained, vec!["two".to_string()]);
        assert!(pending.is_empty());
    }
}
