//! AI System (Milestone 4).
//!
//! Engine-side home for everything AI: the provider abstraction, concrete
//! provider adapters, OS keychain access, SSE framing, error normalization, and
//! (in later waves) conversation persistence. This module — and everything
//! under it — contains ZERO Tauri imports (Requirement 1.2): all Tauri
//! command/event plumbing lives in `frontend/src/main.rs`.

pub mod anthropic;
pub mod context;
pub mod conversation;
pub mod curl_client;
pub mod diff;
pub mod error;
pub mod gemini;
pub mod generic;
pub mod http;
pub mod keychain;
pub mod openai;
pub mod provider;
pub mod registry;
pub mod sse;

pub use diff::{DiffFile, DiffHunk, DiffLine, DiffParseError, looks_like_diff, parse_unified_diff};
pub use error::AiError;
pub use provider::{
    AiProvider, ChatMessage, ChunkSource, ContextAttachment, ImageAttachment, MessageRequest,
    ModelInfo, TokenChunk, TokenStream,
};
