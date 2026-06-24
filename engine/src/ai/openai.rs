//! OpenAI Chat Completions adapter (Requirement 5).
//!
//! Streams `POST /v1/chat/completions` with `stream: true`. The SSE payload is a
//! sequence of `data: {json}` lines (no `event:` names) terminated by
//! `data: [DONE]`. Tokens come from `choices[0].delta.content`; role-only and
//! empty deltas are skipped.
//!
//! The streaming parser ([`interpret_chat_event`]) and stream type
//! ([`OpenAiStream`]) are `pub(crate)` so the Generic OpenAI-compatible adapter
//! can reuse them verbatim (Requirement 7.9).

use std::collections::VecDeque;

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::ai::error::AiError;
use crate::ai::keychain;
use crate::ai::provider::{
    AiProvider, ChunkSource, MessageRequest, ModelInfo, TokenChunk, TokenStream,
};
use crate::ai::sse::SseDecoder;

const CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";
const MODELS_URL: &str = "https://api.openai.com/v1/models";
const KEYCHAIN_ACCOUNT: &str = "openai";

pub struct OpenAiAdapter;

impl OpenAiAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenAiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// What one Chat Completions SSE event means.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ChatAction {
    Text(String),
    /// Structured reasoning text (DeepSeek `reasoning_content`, some Ollama /
    /// OpenAI-compatible `thinking`). Delimited as `<think>...</think>` in the
    /// unified text stream so the frontend separates it (Requirement 7.4).
    Thinking(String),
    Done,
    Ignore,
    Error(AiError),
}

/// Interpret one SSE `data:` payload (already stripped of the `data:` prefix).
/// Shared by OpenAI and Generic adapters.
pub(crate) fn interpret_chat_event(data: &str) -> ChatAction {
    let data = data.trim();
    if data == "[DONE]" {
        return ChatAction::Done;
    }
    match serde_json::from_str::<Value>(data) {
        Ok(v) => {
            if let Some(err) = v.get("error") {
                let msg = err
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("provider error")
                    .to_string();
                return ChatAction::Error(AiError::ProviderError(msg));
            }
            let delta = &v["choices"][0]["delta"];
            // Structured reasoning fields, when the endpoint exposes them.
            let reasoning = delta
                .get("reasoning_content")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    delta
                        .get("thinking")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                });
            if let Some(r) = reasoning {
                return ChatAction::Thinking(r.to_string());
            }
            let text = delta.get("content").and_then(Value::as_str).unwrap_or("");
            if text.is_empty() {
                ChatAction::Ignore // role-only delta or empty content
            } else {
                ChatAction::Text(text.to_string())
            }
        }
        Err(_) => ChatAction::Error(AiError::ProviderError(
            "malformed Chat Completions stream chunk".into(),
        )),
    }
}

/// Convert a `Text`/`Thinking` action into the text fragment to emit, threading
/// `in_thinking` so structured reasoning is wrapped in `<think>...</think>`
/// within the unified text stream. Returns `None` for non-text actions.
pub(crate) fn emit_with_thinking(action: ChatAction, in_thinking: &mut bool) -> Option<String> {
    match action {
        ChatAction::Text(text) => {
            if *in_thinking {
                *in_thinking = false;
                Some(format!("</think>{text}"))
            } else {
                Some(text)
            }
        }
        ChatAction::Thinking(text) => {
            if *in_thinking {
                Some(text)
            } else {
                *in_thinking = true;
                Some(format!("<think>{text}"))
            }
        }
        _ => None,
    }
}

/// Build the JSON request body shared by OpenAI and Generic adapters.
pub(crate) fn build_chat_body(model: &str, request: &MessageRequest) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    if let Some(s) = &request.system
        && !s.is_empty()
    {
        messages.push(json!({ "role": "system", "content": s }));
    }
    let last_idx = request.messages.len().saturating_sub(1);
    for (i, m) in request.messages.iter().enumerate() {
        if i == last_idx && m.role == "user" && !request.images.is_empty() {
            // Multimodal: text part (if any) then image_url data-URLs.
            let mut parts: Vec<Value> = Vec::new();
            if !m.content.is_empty() {
                parts.push(json!({ "type": "text", "text": m.content }));
            }
            for img in &request.images {
                parts.push(json!({
                    "type": "image_url",
                    "image_url": { "url": format!("data:{};base64,{}", img.mime, img.data) },
                }));
            }
            messages.push(json!({ "role": "user", "content": parts }));
        } else {
            messages.push(json!({ "role": m.role, "content": m.content }));
        }
    }
    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    if let Some(max) = request.max_tokens {
        body["max_tokens"] = json!(max);
    }
    body
}

/// Live token stream over a Chat Completions response (OpenAI / Generic).
pub(crate) struct OpenAiStream {
    pub(crate) stream_id: String,
    pub(crate) response: reqwest::Response,
    pub(crate) decoder: SseDecoder,
    pub(crate) pending: VecDeque<TokenChunk>,
    pub(crate) done: bool,
    /// True while inside a structured `<think>` reasoning span.
    pub(crate) in_thinking: bool,
}

impl OpenAiStream {
    pub(crate) fn new(stream_id: String, response: reqwest::Response) -> Self {
        Self {
            stream_id,
            response,
            decoder: SseDecoder::new(),
            pending: VecDeque::new(),
            done: false,
            in_thinking: false,
        }
    }

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
impl ChunkSource for OpenAiStream {
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
                    let events: Vec<_> = self.decoder.push(&bytes);
                    for ev in events {
                        match interpret_chat_event(&ev.data) {
                            action @ (ChatAction::Text(_) | ChatAction::Thinking(_)) => {
                                if let Some(text) =
                                    emit_with_thinking(action, &mut self.in_thinking)
                                {
                                    self.push(text);
                                }
                            }
                            ChatAction::Done => {
                                self.close_thinking();
                                self.done = true;
                            }
                            ChatAction::Ignore => {}
                            ChatAction::Error(e) => {
                                self.done = true;
                                return Err(e);
                            }
                        }
                    }
                }
                Ok(None) => {
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

/// Parse a `/v1/models` response body into `ModelInfo`s, sorted by id. Shared by
/// OpenAI and Generic adapters.
pub(crate) fn parse_models(body: &str) -> Result<Vec<ModelInfo>, AiError> {
    let v: Value = serde_json::from_str(body)
        .map_err(|_| AiError::ProviderError("malformed /v1/models response".into()))?;
    let mut models: Vec<ModelInfo> = v["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["id"].as_str())
                .map(|id| ModelInfo {
                    id: id.to_string(),
                    display_name: id.to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

#[async_trait]
impl AiProvider for OpenAiAdapter {
    async fn send_message(&self, request: MessageRequest) -> Result<TokenStream, AiError> {
        let api_key = keychain::get_api_key(KEYCHAIN_ACCOUNT)?;
        let body = build_chat_body(&request.model, &request);

        let response = reqwest::Client::new()
            .post(CHAT_URL)
            .bearer_auth(api_key)
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
                "OpenAI",
            ));
        }

        Ok(Box::new(OpenAiStream::new(request.stream_id, response)))
    }

    async fn list_models(&self) -> Result<Option<Vec<ModelInfo>>, AiError> {
        let api_key = keychain::get_api_key(KEYCHAIN_ACCOUNT)?;
        let response = reqwest::Client::new()
            .get(MODELS_URL)
            .bearer_auth(api_key)
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
                "OpenAI",
            ));
        }
        let body = response
            .text()
            .await
            .map_err(|e| AiError::Network(e.to_string()))?;
        Ok(Some(parse_models(&body)?))
    }

    fn provider_name(&self) -> &'static str {
        "openai"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::{ChatMessage, ImageAttachment};

    #[test]
    fn emits_text_delta_content() {
        let data = r#"{"choices":[{"delta":{"content":"Hello"},"index":0}]}"#;
        assert_eq!(interpret_chat_event(data), ChatAction::Text("Hello".into()));
    }

    #[test]
    fn ignores_role_only_delta() {
        let data = r#"{"choices":[{"delta":{"role":"assistant"},"index":0}]}"#;
        assert_eq!(interpret_chat_event(data), ChatAction::Ignore);
    }

    #[test]
    fn maps_reasoning_content_to_thinking() {
        let data = r#"{"choices":[{"delta":{"reasoning_content":"Let me think"},"index":0}]}"#;
        assert_eq!(
            interpret_chat_event(data),
            ChatAction::Thinking("Let me think".into())
        );
    }

    #[test]
    fn maps_thinking_field_to_thinking() {
        let data = r#"{"choices":[{"delta":{"thinking":"weighing options"},"index":0}]}"#;
        assert_eq!(
            interpret_chat_event(data),
            ChatAction::Thinking("weighing options".into())
        );
    }

    #[test]
    fn prefers_content_when_reasoning_empty() {
        let data = r#"{"choices":[{"delta":{"reasoning_content":"","content":"Hi"},"index":0}]}"#;
        assert_eq!(interpret_chat_event(data), ChatAction::Text("Hi".into()));
    }

    #[test]
    fn emit_with_thinking_wraps_reasoning_then_answer() {
        let mut inside = false;
        assert_eq!(
            emit_with_thinking(ChatAction::Thinking("Let me".into()), &mut inside),
            Some("<think>Let me".into())
        );
        assert!(inside);
        assert_eq!(
            emit_with_thinking(ChatAction::Thinking(" decide".into()), &mut inside),
            Some(" decide".into())
        );
        assert!(inside);
        assert_eq!(
            emit_with_thinking(ChatAction::Text("Answer".into()), &mut inside),
            Some("</think>Answer".into())
        );
        assert!(!inside);
        assert_eq!(
            emit_with_thinking(ChatAction::Text(" more".into()), &mut inside),
            Some(" more".into())
        );
    }

    #[test]
    fn ignores_null_content_delta() {
        let data = r#"{"choices":[{"delta":{"content":null},"index":0}]}"#;
        assert_eq!(interpret_chat_event(data), ChatAction::Ignore);
    }

    #[test]
    fn ignores_empty_content_delta() {
        let data = r#"{"choices":[{"delta":{"content":""},"index":0}]}"#;
        assert_eq!(interpret_chat_event(data), ChatAction::Ignore);
    }

    #[test]
    fn stops_on_done_sentinel() {
        assert_eq!(interpret_chat_event("[DONE]"), ChatAction::Done);
        assert_eq!(interpret_chat_event(" [DONE] "), ChatAction::Done);
    }

    #[test]
    fn malformed_json_is_provider_error() {
        assert!(matches!(
            interpret_chat_event("{nope"),
            ChatAction::Error(AiError::ProviderError(_))
        ));
    }

    #[test]
    fn error_object_in_stream_is_provider_error() {
        let data = r#"{"error":{"message":"model overloaded","type":"server_error"}}"#;
        match interpret_chat_event(data) {
            ChatAction::Error(AiError::ProviderError(m)) => assert!(m.contains("overloaded")),
            other => panic!("expected provider error, got {other:?}"),
        }
    }

    #[test]
    fn multiple_chunks_concatenate() {
        let chunks = [
            r#"{"choices":[{"delta":{"role":"assistant"}}]}"#,
            r#"{"choices":[{"delta":{"content":"Hel"}}]}"#,
            r#"{"choices":[{"delta":{"content":"lo"}}]}"#,
            "[DONE]",
        ];
        let mut out = String::new();
        let mut done = false;
        for c in chunks {
            match interpret_chat_event(c) {
                ChatAction::Text(t) => out.push_str(&t),
                ChatAction::Thinking(t) => out.push_str(&t),
                ChatAction::Done => done = true,
                ChatAction::Ignore => {}
                ChatAction::Error(e) => panic!("{e:?}"),
            }
        }
        assert_eq!(out, "Hello");
        assert!(done);
    }

    #[test]
    fn parse_models_sorts_ids() {
        let body = r#"{"data":[{"id":"gpt-4o"},{"id":"gpt-3.5-turbo"},{"id":"o1"}]}"#;
        let models = parse_models(body).unwrap();
        let ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["gpt-3.5-turbo", "gpt-4o", "o1"]);
    }

    #[test]
    fn build_chat_body_includes_system_and_messages() {
        let req = MessageRequest {
            stream_id: "s".into(),
            messages: vec![ChatMessage::user("hi")],
            system: Some("be terse".into()),
            attachments: vec![],
            images: vec![],
            model: "gpt-4o".into(),
            max_tokens: Some(256),
        };
        let body = build_chat_body("gpt-4o", &req);
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 256);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "be terse");
        assert_eq!(body["messages"][1]["role"], "user");
    }

    #[test]
    fn build_chat_body_attaches_image_to_last_user_turn() {
        let req = MessageRequest {
            stream_id: "s".into(),
            messages: vec![ChatMessage::user("caption this")],
            system: None,
            attachments: vec![],
            images: vec![ImageAttachment {
                mime: "image/jpeg".into(),
                data: "Zm9v".into(),
            }],
            model: "gpt-4o".into(),
            max_tokens: None,
        };
        let body = build_chat_body("gpt-4o", &req);
        let content = &body["messages"][0]["content"];
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "caption this");
        assert_eq!(content[1]["type"], "image_url");
        assert_eq!(
            content[1]["image_url"]["url"],
            "data:image/jpeg;base64,Zm9v"
        );
    }
}
