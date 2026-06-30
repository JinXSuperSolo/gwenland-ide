//! Google Gemini adapter (Requirement 6).
//!
//! Streams `:streamGenerateContent?alt=sse`. Each SSE event carries a
//! `GenerateContentResponse` whose `candidates[0].content.parts[].text` values
//! are concatenated in order. The stream ends when a `finishReason` is present
//! or the connection closes.
//!
//! Auth uses the `x-goog-api-key` header (NOT the `?key=` query form) so the key
//! never appears in a URL that could surface in a transport error string.
//! Gemini roles differ from ChatML: `assistant` maps to `model`; the system
//! prompt goes in a dedicated `systemInstruction` field.

use std::collections::{HashMap, VecDeque};

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::ai::curl_client::{CurlStream, curl_get, curl_post_stream};
use crate::ai::error::AiError;
use crate::ai::keychain;
use crate::ai::provider::{
    AiProvider, ChunkSource, MessageRequest, ModelInfo, TokenChunk, TokenStream,
};
use crate::ai::sse::SseDecoder;

const BASE: &str = "https://generativelanguage.googleapis.com/v1beta";
const KEYCHAIN_ACCOUNT: &str = "gemini";

pub struct GeminiAdapter;

impl GeminiAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GeminiAction {
    /// Text from this event, plus whether a finish reason ended the turn.
    Chunk {
        text: String,
        finished: bool,
    },
    Ignore,
    Error(AiError),
}

/// Interpret one `streamGenerateContent` SSE `data:` payload.
pub(crate) fn interpret_event(data: &str) -> GeminiAction {
    let data = data.trim();
    if data.is_empty() {
        return GeminiAction::Ignore;
    }
    match serde_json::from_str::<Value>(data) {
        Ok(v) => {
            if let Some(err) = v.get("error") {
                let msg = err
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("provider error")
                    .to_string();
                return GeminiAction::Error(AiError::ProviderError(msg));
            }
            let candidate = &v["candidates"][0];
            let mut text = String::new();
            if let Some(parts) = candidate["content"]["parts"].as_array() {
                for p in parts {
                    if let Some(t) = p["text"].as_str() {
                        text.push_str(t);
                    }
                }
            }
            let finished = candidate
                .get("finishReason")
                .and_then(Value::as_str)
                .is_some();
            GeminiAction::Chunk { text, finished }
        }
        Err(_) => GeminiAction::Error(AiError::ProviderError(
            "malformed Gemini stream chunk".into(),
        )),
    }
}

/// Translate ChatML messages into Gemini `contents`. `assistant` -> `model`.
/// Images attach to the last (current) user turn as `inline_data` parts.
fn build_contents(request: &MessageRequest) -> Vec<Value> {
    let msgs: Vec<_> = request
        .messages
        .iter()
        .filter(|m| m.role != "system")
        .collect();
    let last_idx = msgs.len().saturating_sub(1);
    msgs.iter()
        .enumerate()
        .map(|(i, m)| {
            let role = if m.role == "assistant" {
                "model"
            } else {
                "user"
            };
            if i == last_idx && m.role == "user" && !request.images.is_empty() {
                let mut parts: Vec<Value> = request
                    .images
                    .iter()
                    .map(
                        |img| json!({ "inline_data": { "mime_type": img.mime, "data": img.data } }),
                    )
                    .collect();
                if !m.content.is_empty() {
                    parts.push(json!({ "text": m.content }));
                }
                json!({ "role": role, "parts": parts })
            } else {
                json!({ "role": role, "parts": [{ "text": m.content }] })
            }
        })
        .collect()
}

struct GeminiStream {
    stream_id: String,
    response: CurlStream,
    decoder: SseDecoder,
    pending: VecDeque<TokenChunk>,
    done: bool,
}

#[async_trait]
impl ChunkSource for GeminiStream {
    async fn next_chunk(&mut self) -> Result<Option<TokenChunk>, AiError> {
        loop {
            if let Some(chunk) = self.pending.pop_front() {
                return Ok(Some(chunk));
            }
            if self.done {
                return Ok(None);
            }
            match self.response.next_bytes().await {
                Ok(Some(bytes)) => {
                    for ev in self.decoder.push(&bytes) {
                        match interpret_event(&ev.data) {
                            GeminiAction::Chunk { text, finished } => {
                                if !text.is_empty() {
                                    self.pending.push_back(TokenChunk {
                                        stream_id: self.stream_id.clone(),
                                        text,
                                    });
                                }
                                if finished {
                                    self.done = true;
                                }
                            }
                            GeminiAction::Ignore => {}
                            GeminiAction::Error(e) => {
                                self.done = true;
                                return Err(e);
                            }
                        }
                    }
                }
                Ok(None) => self.done = true,
                Err(e) => {
                    self.done = true;
                    return Err(AiError::Network(e.to_string()));
                }
            }
        }
    }
}

#[async_trait]
impl AiProvider for GeminiAdapter {
    async fn send_message(&self, request: MessageRequest) -> Result<TokenStream, AiError> {
        let api_key = keychain::get_api_key(KEYCHAIN_ACCOUNT)?;
        let url = format!(
            "{BASE}/models/{}:streamGenerateContent?alt=sse",
            request.model
        );

        let mut body = json!({ "contents": build_contents(&request) });
        if let Some(s) = &request.system
            && !s.is_empty()
        {
            body["systemInstruction"] = json!({ "parts": [{ "text": s }] });
        }
        if let Some(max) = request.max_tokens {
            body["generationConfig"] = json!({ "maxOutputTokens": max });
        }

        let body = serde_json::to_string(&body)
            .map_err(|_| AiError::ProviderError("failed to serialize Gemini request".into()))?;
        let mut headers = HashMap::new();
        headers.insert("x-goog-api-key".to_string(), api_key);
        headers.insert("content-type".to_string(), "application/json".to_string());

        let response = curl_post_stream(&url, &headers, body).await?;
        if !(200..300).contains(&response.status) {
            return Err(crate::ai::http::map_stream_error(
                response.status,
                &response.headers,
                response.body,
                "Gemini",
            )
            .await);
        }

        Ok(Box::new(GeminiStream {
            stream_id: request.stream_id,
            response: response.body,
            decoder: SseDecoder::new(),
            pending: VecDeque::new(),
            done: false,
        }))
    }

    async fn list_models(&self) -> Result<Option<Vec<ModelInfo>>, AiError> {
        let api_key = keychain::get_api_key(KEYCHAIN_ACCOUNT)?;
        let mut headers = HashMap::new();
        headers.insert("x-goog-api-key".to_string(), api_key);
        let response = curl_get(&format!("{BASE}/models"), &headers).await?;

        if !(200..300).contains(&response.status) {
            let retry_after = crate::ai::http::retry_after_secs(&response.headers);
            return Err(crate::ai::http::map_status(
                response.status,
                retry_after,
                &response.body,
                "Gemini",
            ));
        }
        Ok(Some(parse_models(&response.body)))
    }

    fn provider_name(&self) -> &'static str {
        "gemini"
    }
}

/// Parse a Gemini `models` listing, keeping only models that support
/// `generateContent` and stripping the `models/` id prefix.
fn parse_models(body: &str) -> Vec<ModelInfo> {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    v["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|m| {
                    m["supportedGenerationMethods"]
                        .as_array()
                        .map(|methods| {
                            methods
                                .iter()
                                .any(|x| x.as_str() == Some("generateContent"))
                        })
                        .unwrap_or(false)
                })
                .filter_map(|m| {
                    let name = m["name"].as_str()?;
                    let id = name.strip_prefix("models/").unwrap_or(name).to_string();
                    let display_name = m["displayName"].as_str().unwrap_or(&id).to_string();
                    Some(ModelInfo { id, display_name })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::{ChatMessage, ImageAttachment};

    #[test]
    fn concatenates_multi_part_text() {
        let data = r#"{"candidates":[{"content":{"parts":[{"text":"Hel"},{"text":"lo"}]}}]}"#;
        assert_eq!(
            interpret_event(data),
            GeminiAction::Chunk {
                text: "Hello".into(),
                finished: false
            }
        );
    }

    #[test]
    fn marks_finished_on_finish_reason() {
        let data = r#"{"candidates":[{"content":{"parts":[{"text":"."}]},"finishReason":"STOP"}]}"#;
        assert_eq!(
            interpret_event(data),
            GeminiAction::Chunk {
                text: ".".into(),
                finished: true
            }
        );
    }

    #[test]
    fn empty_payload_is_ignored() {
        assert_eq!(interpret_event("  "), GeminiAction::Ignore);
    }

    #[test]
    fn malformed_json_is_provider_error() {
        assert!(matches!(
            interpret_event("{oops"),
            GeminiAction::Error(AiError::ProviderError(_))
        ));
    }

    #[test]
    fn error_object_maps_to_provider_error() {
        let data = r#"{"error":{"code":400,"message":"bad request"}}"#;
        match interpret_event(data) {
            GeminiAction::Error(AiError::ProviderError(m)) => assert!(m.contains("bad request")),
            other => panic!("expected provider error, got {other:?}"),
        }
    }

    #[test]
    fn build_contents_maps_assistant_to_model_and_drops_system() {
        let req = MessageRequest {
            stream_id: "s".into(),
            messages: vec![
                ChatMessage::system("ignored here"),
                ChatMessage::user("hi"),
                ChatMessage::assistant("hello"),
            ],
            system: Some("be terse".into()),
            attachments: vec![],
            images: vec![],
            model: "gemini-2.0-flash".into(),
            max_tokens: None,
        };
        let contents = build_contents(&req);
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(contents[1]["parts"][0]["text"], "hello");
    }

    #[test]
    fn build_contents_attaches_image_to_last_user_turn() {
        let req = MessageRequest {
            stream_id: "s".into(),
            messages: vec![ChatMessage::user("what is this?")],
            system: None,
            attachments: vec![],
            images: vec![ImageAttachment {
                mime: "image/png".into(),
                data: "QUJD".into(),
            }],
            model: "gemini-2.0-flash".into(),
            max_tokens: None,
        };
        let contents = build_contents(&req);
        let parts = contents[0]["parts"].as_array().unwrap();
        assert_eq!(parts[0]["inline_data"]["mime_type"], "image/png");
        assert_eq!(parts[0]["inline_data"]["data"], "QUJD");
        assert_eq!(parts[1]["text"], "what is this?");
    }

    #[test]
    fn parse_models_filters_and_strips_prefix() {
        let body = r#"{"models":[
            {"name":"models/gemini-2.0-flash","displayName":"Gemini 2.0 Flash","supportedGenerationMethods":["generateContent"]},
            {"name":"models/embedding-001","displayName":"Embedding","supportedGenerationMethods":["embedContent"]}
        ]}"#;
        let models = parse_models(body);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gemini-2.0-flash");
        assert_eq!(models[0].display_name, "Gemini 2.0 Flash");
    }
}
