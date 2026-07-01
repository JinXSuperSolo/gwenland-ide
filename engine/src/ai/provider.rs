//! Provider-neutral AI interface and core types (Requirement 2).
//!
//! Every adapter (Anthropic, OpenAI, Gemini, Generic) implements [`AiProvider`].
//! The command layer never depends on which provider is active: it builds a
//! [`MessageRequest`], calls [`AiProvider::send_message`], and drains the
//! returned [`TokenStream`] one [`TokenChunk`] at a time.
//!
//! API keys deliberately appear in NONE of these structs — adapters fetch their
//! key from the OS keychain at send time (see [`crate::ai::keychain`]).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::ai::error::AiError;

/// A single streamed text fragment correlated to its request via `stream_id`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenChunk {
    pub stream_id: String,
    pub text: String,
}

/// Token accounting for one completed request, when the provider reports it
/// (GWEN-457). Captured by the adapter as it reads terminal stream metadata
/// (Anthropic's `message_start`/`message_delta` usage fields, OpenAI's
/// `stream_options.include_usage` final chunk, Gemini's `usageMetadata`) and
/// retrieved via [`ChunkSource::usage`] once the stream is exhausted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// A pull-based stream of tokens.
///
/// We model the stream as a boxed trait object with an async `next_chunk`
/// method rather than a `futures::Stream`. This keeps the engine free of stream
/// combinator dependencies while staying object-safe (each adapter owns its own
/// parsing state + the live transport response). The consumer loops:
///
/// ```ignore
/// while let Some(chunk) = stream.next_chunk().await? { emit(chunk); }
/// ```
#[async_trait]
pub trait ChunkSource: Send {
    /// Returns the next token, `Ok(None)` at clean end of stream, or a
    /// normalized [`AiError`]. After `Ok(None)` or `Err(..)`, callers must not
    /// call again.
    async fn next_chunk(&mut self) -> Result<Option<TokenChunk>, AiError>;

    /// Token usage for this request, if the provider reported it. Only
    /// meaningful after `next_chunk` has returned `Ok(None)` — usage arrives as
    /// terminal stream metadata, not a chunk of its own. Adapters that don't
    /// capture usage (or providers that never return it) default to `None`.
    fn usage(&self) -> Option<TokenUsage> {
        None
    }
}

/// Boxed token stream returned by [`AiProvider::send_message`].
pub type TokenStream = Box<dyn ChunkSource>;

/// One ChatML-compatible message. `role` is one of `system`, `user`,
/// `assistant`; persisted verbatim to JSONL (Requirement 16.5).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

/// Extra context attached to a user message (Requirement 2.7 / 14).
///
/// Serde-tagged with `snake_case` variant names so the wire form is
/// `{"type":"file",...}`, `{"type":"selection",...}`,
/// `{"type":"terminal_error",...}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextAttachment {
    /// A whole file under the active project root. The engine reads its
    /// contents as UTF-8 at send time (Wave 5); `path` is project-relative or
    /// absolute as provided by the UI.
    File { path: String },
    /// An editor selection. The UI supplies the `content` (the selected text)
    /// and a non-empty source `path` (Requirement 14.7).
    Selection { path: String, content: String },
    /// A terminal error line surfaced by the M3 `terminal://error` bridge. The
    /// `label` and `line` are exactly as emitted by M3 (Requirement 15.7).
    TerminalError { label: String, line: String },
}

/// Everything an adapter needs to start a streaming completion. Carries no key.
#[derive(Debug, Clone)]
pub struct MessageRequest {
    /// Frontend-generated UUID correlating every event for this request.
    pub stream_id: String,
    /// Prior turns plus the current user message, in order.
    pub messages: Vec<ChatMessage>,
    /// Optional system prompt (kept separate from `messages` for providers like
    /// Anthropic and Gemini that take it as a dedicated field).
    pub system: Option<String>,
    /// Attachments to expand into the final user message.
    pub attachments: Vec<ContextAttachment>,
    /// Images attached to the current user turn (multimodal); adapters append
    /// these to the last user message in their own format. Empty for text-only.
    pub images: Vec<ImageAttachment>,
    /// The model id to call.
    pub model: String,
    /// Optional cap on generated tokens.
    pub max_tokens: Option<u32>,
}

/// A model offered by a provider, for the model picker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
}

/// An image attached to the current user turn (Milestone 8 multimodal). Carries
/// the raw bytes base64-encoded plus the MIME type; adapters render it in their
/// own wire format. These are NOT persisted to conversation JSONL — they apply
/// only to the request being sent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageAttachment {
    /// MIME type, e.g. `image/png`, `image/jpeg`, `image/webp`.
    pub mime: String,
    /// Base64-encoded image bytes (no `data:` prefix).
    pub data: String,
}

/// The interface every provider adapter implements.
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Open a streaming completion. Initial transport/auth errors surface here;
    /// per-token errors surface from [`ChunkSource::next_chunk`].
    async fn send_message(&self, request: MessageRequest) -> Result<TokenStream, AiError>;

    /// List available models. `Ok(None)` means the provider does not support
    /// listing (e.g. a Generic endpoint without `/v1/models`).
    async fn list_models(&self) -> Result<Option<Vec<ModelInfo>>, AiError>;

    /// Stable provider name for diagnostics.
    fn provider_name(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_attachment_uses_snake_case_type_tags() {
        let file = serde_json::to_value(ContextAttachment::File {
            path: "src/main.rs".into(),
        })
        .unwrap();
        assert_eq!(file["type"], "file");

        let sel = serde_json::to_value(ContextAttachment::Selection {
            path: "a.rs".into(),
            content: "x".into(),
        })
        .unwrap();
        assert_eq!(sel["type"], "selection");

        let term = serde_json::to_value(ContextAttachment::TerminalError {
            label: "rust-panic".into(),
            line: "panicked at".into(),
        })
        .unwrap();
        assert_eq!(term["type"], "terminal_error");
    }

    #[test]
    fn context_attachment_round_trips() {
        let original = ContextAttachment::Selection {
            path: "src/lib.rs".into(),
            content: "fn main() {}\n".into(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: ContextAttachment = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }
}
