//! Anthropic Messages API adapter (Requirement 4).
//!
//! Streams Claude responses via the Messages API with `stream: true`, parsing
//! the SSE event sequence (`message_start`, `content_block_start`, `ping`,
//! `content_block_delta`, `content_block_stop`, `message_delta`, `message_stop`,
//! `error`). Only `content_block_delta` events whose `delta.type` is
//! `text_delta` produce tokens; ping/keepalive and structural events are
//! ignored; `message_stop` ends the stream cleanly.

use std::collections::VecDeque;

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::ai::error::AiError;
use crate::ai::keychain;
use crate::ai::provider::{
    AiProvider, ChunkSource, MessageRequest, ModelInfo, TokenChunk, TokenStream,
};
use crate::ai::sse::{SseDecoder, SseEvent};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const KEYCHAIN_ACCOUNT: &str = "anthropic";
const DEFAULT_MAX_TOKENS: u32 = 4096;

pub struct AnthropicAdapter;

impl AnthropicAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AnthropicAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// What a single parsed SSE event means to the Anthropic stream.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Text(String),
    /// Extended-thinking text (`thinking_delta`), delimited as `<think>...
    /// </think>` in the unified text stream so the frontend separates it
    /// (Requirement 7.5).
    Thinking(String),
    Stop,
    Ignore,
    Error(AiError),
}

/// Pure interpretation of one framed SSE event — kept separate from the network
/// loop so it can be unit-tested with synthetic events (Requirement 8.1/8.5).
fn interpret_event(ev: &SseEvent) -> Action {
    match ev.event.as_deref() {
        Some("ping") => Action::Ignore,
        Some("message_stop") => Action::Stop,
        Some("error") => match serde_json::from_str::<Value>(&ev.data) {
            Ok(v) => Action::Error(map_error_event(&v)),
            Err(_) => Action::Error(AiError::ProviderError(
                "malformed Anthropic error event".into(),
            )),
        },
        Some("content_block_delta") => match serde_json::from_str::<Value>(&ev.data) {
            Ok(v) => {
                let dtype = v
                    .get("delta")
                    .and_then(|d| d.get("type"))
                    .and_then(Value::as_str);
                match dtype {
                    Some("text_delta") => {
                        let text = v["delta"]["text"].as_str().unwrap_or("").to_string();
                        if text.is_empty() {
                            Action::Ignore
                        } else {
                            Action::Text(text)
                        }
                    }
                    Some("thinking_delta") => {
                        let text = v["delta"]["thinking"].as_str().unwrap_or("").to_string();
                        if text.is_empty() {
                            Action::Ignore
                        } else {
                            Action::Thinking(text)
                        }
                    }
                    // e.g. input_json_delta (tool use), signature_delta — unused in M4.
                    _ => Action::Ignore,
                }
            }
            Err(_) => Action::Error(AiError::ProviderError(
                "malformed Anthropic content_block_delta".into(),
            )),
        },
        // message_start, content_block_start/stop, message_delta, unknown.
        _ => Action::Ignore,
    }
}

/// Map an Anthropic `error` event body to a normalized [`AiError`] without
/// leaking keys or headers.
fn map_error_event(v: &Value) -> AiError {
    let etype = v["error"]["type"].as_str().unwrap_or("");
    let msg = v["error"]["message"]
        .as_str()
        .unwrap_or("provider error")
        .to_string();
    match etype {
        "authentication_error" | "permission_error" => AiError::InvalidKey,
        "rate_limit_error" => AiError::RateLimit { retry_after: None },
        "overloaded_error" => AiError::ProviderError(msg),
        _ if msg.to_lowercase().contains("context") || msg.to_lowercase().contains("too long") => {
            AiError::ContextLengthExceeded
        }
        _ => AiError::ProviderError(msg),
    }
}

/// Live token stream over a Messages API response.
struct AnthropicStream {
    stream_id: String,
    response: reqwest::Response,
    decoder: SseDecoder,
    pending: VecDeque<TokenChunk>,
    done: bool,
    /// True while inside an extended-thinking `<think>` span.
    in_thinking: bool,
}

impl AnthropicStream {
    fn push(&mut self, text: String) {
        self.pending.push_back(TokenChunk {
            stream_id: self.stream_id.clone(),
            text,
        });
    }

    /// Close an open `<think>` span if the stream ends mid-reasoning.
    fn close_thinking(&mut self) {
        if self.in_thinking {
            self.in_thinking = false;
            self.push("</think>".into());
        }
    }
}

#[async_trait]
impl ChunkSource for AnthropicStream {
    async fn next_chunk(&mut self) -> Result<Option<TokenChunk>, AiError> {
        loop {
            if let Some(chunk) = self.pending.pop_front() {
                return Ok(Some(chunk));
            }
            if self.done {
                return Ok(None);
            }
            match self.response.chunk().await {
                Ok(Some(bytes)) => {
                    let events = self.decoder.push(&bytes);
                    for ev in events {
                        match interpret_event(&ev) {
                            Action::Text(text) => {
                                if self.in_thinking {
                                    self.in_thinking = false;
                                    self.push(format!("</think>{text}"));
                                } else {
                                    self.push(text);
                                }
                            }
                            Action::Thinking(text) => {
                                if self.in_thinking {
                                    self.push(text);
                                } else {
                                    self.in_thinking = true;
                                    self.push(format!("<think>{text}"));
                                }
                            }
                            Action::Stop => {
                                self.close_thinking();
                                self.done = true;
                            }
                            Action::Ignore => {}
                            Action::Error(e) => {
                                self.done = true;
                                return Err(e);
                            }
                        }
                    }
                }
                Ok(None) => {
                    // EOF without explicit message_stop.
                    self.close_thinking();
                    self.done = true;
                }
                Err(e) => {
                    self.done = true;
                    return Err(AiError::Network(e.to_string()));
                }
            }
        }
    }
}

#[async_trait]
impl AiProvider for AnthropicAdapter {
    async fn send_message(&self, request: MessageRequest) -> Result<TokenStream, AiError> {
        let api_key = keychain::get_api_key(KEYCHAIN_ACCOUNT)?;

        // Anthropic takes `system` as a top-level field, not a message role.
        let mut system_parts: Vec<String> = Vec::new();
        if let Some(s) = &request.system
            && !s.is_empty()
        {
            system_parts.push(s.clone());
        }
        let mut messages = Vec::new();
        let last_idx = request.messages.len().saturating_sub(1);
        for (i, m) in request.messages.iter().enumerate() {
            if m.role == "system" {
                system_parts.push(m.content.clone());
            } else if i == last_idx && m.role == "user" && !request.images.is_empty() {
                // Multimodal: image blocks first, then the text (if any).
                let mut content: Vec<Value> = request
                    .images
                    .iter()
                    .map(|img| {
                        json!({
                            "type": "image",
                            "source": { "type": "base64", "media_type": img.mime, "data": img.data },
                        })
                    })
                    .collect();
                if !m.content.is_empty() {
                    content.push(json!({ "type": "text", "text": m.content }));
                }
                messages.push(json!({ "role": "user", "content": content }));
            } else {
                messages.push(json!({ "role": m.role, "content": m.content }));
            }
        }

        let mut body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            "messages": messages,
            "stream": true,
        });
        if !system_parts.is_empty() {
            body["system"] = Value::String(system_parts.join("\n\n"));
        }

        let response = reqwest::Client::new()
            .post(API_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AiError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let retry_after = crate::ai::http::retry_after_secs(response.headers());
            let body = response.text().await.unwrap_or_default();
            return Err(crate::ai::http::map_status(
                status,
                retry_after,
                &body,
                "Anthropic",
            ));
        }

        Ok(Box::new(AnthropicStream {
            stream_id: request.stream_id,
            response,
            decoder: SseDecoder::new(),
            pending: VecDeque::new(),
            done: false,
            in_thinking: false,
        }))
    }

    async fn list_models(&self) -> Result<Option<Vec<ModelInfo>>, AiError> {
        // Curated static list (Requirement 4.9). Latest, most capable first.
        Ok(Some(vec![
            ModelInfo {
                id: "claude-opus-4-8".into(),
                display_name: "Claude Opus 4.8".into(),
            },
            ModelInfo {
                id: "claude-sonnet-4-6".into(),
                display_name: "Claude Sonnet 4.6".into(),
            },
            ModelInfo {
                id: "claude-haiku-4-5-20251001".into(),
                display_name: "Claude Haiku 4.5".into(),
            },
        ]))
    }

    fn provider_name(&self) -> &'static str {
        "anthropic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::sse::SseEvent;

    fn ev(name: &str, data: &str) -> SseEvent {
        SseEvent {
            event: Some(name.to_string()),
            data: data.to_string(),
        }
    }

    #[test]
    fn emits_text_for_text_delta() {
        let e = ev(
            "content_block_delta",
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#,
        );
        assert_eq!(interpret_event(&e), Action::Text("Hello".into()));
    }

    #[test]
    fn ignores_empty_text_delta() {
        let e = ev(
            "content_block_delta",
            r#"{"delta":{"type":"text_delta","text":""}}"#,
        );
        assert_eq!(interpret_event(&e), Action::Ignore);
    }

    #[test]
    fn ignores_non_text_delta() {
        let e = ev(
            "content_block_delta",
            r#"{"delta":{"type":"input_json_delta","partial_json":"{"}}"#,
        );
        assert_eq!(interpret_event(&e), Action::Ignore);
    }

    #[test]
    fn emits_thinking_for_thinking_delta() {
        let e = ev(
            "content_block_delta",
            r#"{"delta":{"type":"thinking_delta","thinking":"Let me reason"}}"#,
        );
        assert_eq!(
            interpret_event(&e),
            Action::Thinking("Let me reason".into())
        );
    }

    #[test]
    fn ignores_empty_thinking_delta() {
        let e = ev(
            "content_block_delta",
            r#"{"delta":{"type":"thinking_delta","thinking":""}}"#,
        );
        assert_eq!(interpret_event(&e), Action::Ignore);
    }

    #[test]
    fn ignores_signature_delta() {
        let e = ev(
            "content_block_delta",
            r#"{"delta":{"type":"signature_delta","signature":"abc123"}}"#,
        );
        assert_eq!(interpret_event(&e), Action::Ignore);
    }

    #[test]
    fn ignores_ping() {
        assert_eq!(interpret_event(&ev("ping", "{}")), Action::Ignore);
    }

    #[test]
    fn ignores_structural_events() {
        assert_eq!(
            interpret_event(&ev("message_start", r#"{"type":"message_start"}"#)),
            Action::Ignore
        );
        assert_eq!(
            interpret_event(&ev("content_block_start", "{}")),
            Action::Ignore
        );
    }

    #[test]
    fn stops_on_message_stop() {
        assert_eq!(interpret_event(&ev("message_stop", "{}")), Action::Stop);
    }

    #[test]
    fn malformed_json_in_delta_is_provider_error() {
        let e = ev("content_block_delta", "{not json");
        assert!(matches!(
            interpret_event(&e),
            Action::Error(AiError::ProviderError(_))
        ));
    }

    #[test]
    fn error_event_maps_to_normalized_error() {
        let e = ev(
            "error",
            r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
        );
        assert_eq!(interpret_event(&e), Action::Error(AiError::InvalidKey));

        let e = ev(
            "error",
            r#"{"error":{"type":"rate_limit_error","message":"slow down"}}"#,
        );
        assert_eq!(
            interpret_event(&e),
            Action::Error(AiError::RateLimit { retry_after: None })
        );
    }

    #[test]
    fn full_stream_via_decoder_yields_concatenated_text() {
        // Feed a realistic Anthropic SSE sequence through the shared decoder and
        // collect the resulting actions.
        let raw = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\"}\n\n",
            "event: ping\ndata: {\"type\":\"ping\"}\n\n",
            "event: content_block_delta\ndata: {\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n",
            "event: content_block_delta\ndata: {\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        let mut decoder = SseDecoder::new();
        let mut out = String::new();
        let mut stopped = false;
        for e in decoder.push(raw.as_bytes()) {
            match interpret_event(&e) {
                Action::Text(t) => out.push_str(&t),
                Action::Thinking(t) => out.push_str(&t),
                Action::Stop => stopped = true,
                Action::Ignore => {}
                Action::Error(e) => panic!("unexpected error: {e:?}"),
            }
        }
        assert_eq!(out, "Hello");
        assert!(stopped);
    }
}
