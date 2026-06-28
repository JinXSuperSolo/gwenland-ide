//! Generic OpenAI-compatible adapter (Requirement 7).
//!
//! One configurable adapter for Groq, DeepSeek, Mistral, OpenRouter, Together,
//! Ollama, LM Studio, and any other endpoint speaking the OpenAI Chat
//! Completions wire format. All provider-specific quirks live in config
//! (`base_url`, `default_model`, `extra_headers`) rather than bespoke branches —
//! e.g. OpenRouter attribution via `HTTP-Referer` / `X-Title` extra headers.
//!
//! It reuses the OpenAI request body, SSE parser, stream type, and `/v1/models`
//! parser verbatim (Requirement 7.9). The key is fetched from the keychain
//! account matching the generic provider id.

use std::collections::BTreeMap;

use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

use crate::ai::error::AiError;
use crate::ai::keychain;
use crate::ai::openai::{OpenAiStream, build_chat_body, parse_models};
use crate::ai::provider::{AiProvider, MessageRequest, ModelInfo, TokenStream};
use crate::system::settings::GenericProviderSetting;

pub struct GenericAdapter {
    /// Provider id (e.g. `generic-groq`); also the keychain account id.
    pub id: String,
    pub base_url: String,
    pub default_model: String,
    pub extra_headers: BTreeMap<String, String>,
}

impl GenericAdapter {
    pub fn from_setting(setting: &GenericProviderSetting) -> Self {
        Self {
            id: setting.id.clone(),
            base_url: setting.base_url.clone(),
            default_model: setting.default_model.clone(),
            extra_headers: setting.extra_headers.clone(),
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), path)
    }
}

/// Build the request headers: JSON content-type, Bearer auth, then every
/// configured extra header (forwarded without hardcoding any provider branch).
pub(crate) fn build_headers(
    api_key: &str,
    extra: &BTreeMap<String, String>,
) -> Result<HeaderMap, AiError> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let auth = format!("Bearer {api_key}");
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth)
            .map_err(|_| AiError::ProviderError("invalid characters in API key".into()))?,
    );
    for (k, v) in extra {
        let name = HeaderName::from_bytes(k.as_bytes())
            .map_err(|_| AiError::ProviderError(format!("invalid extra header name: {k}")))?;
        let value = HeaderValue::from_str(v)
            .map_err(|_| AiError::ProviderError(format!("invalid extra header value for {k}")))?;
        headers.insert(name, value);
    }
    Ok(headers)
}

#[async_trait]
impl AiProvider for GenericAdapter {
    async fn send_message(&self, request: MessageRequest) -> Result<TokenStream, AiError> {
        let api_key = keychain::get_api_key(&self.id)?;
        let model = if request.model.is_empty() {
            self.default_model.clone()
        } else {
            request.model.clone()
        };
        let headers = build_headers(&api_key, &self.extra_headers)?;
        let body = build_chat_body(&model, &request);

        let response = reqwest::Client::new()
            .post(self.endpoint("chat/completions"))
            .headers(headers)
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
                &self.id,
            ));
        }

        Ok(Box::new(OpenAiStream::new(request.stream_id, response)))
    }

    async fn list_models(&self) -> Result<Option<Vec<ModelInfo>>, AiError> {
        // Best-effort: many compatible endpoints lack /v1/models. Any failure
        // (network, non-2xx, unparseable) is reported as "unsupported" so the UI
        // can fall back to the configured default model (Requirement 7.7).
        let api_key = keychain::get_api_key(&self.id)?;
        let headers = match build_headers(&api_key, &self.extra_headers) {
            Ok(h) => h,
            Err(_) => return Ok(None),
        };
        let response = match reqwest::Client::new()
            .get(self.endpoint("models"))
            .headers(headers)
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if !response.status().is_success() {
            return Ok(None);
        }
        match response.text().await {
            Ok(body) => Ok(parse_models(&body).ok()),
            Err(_) => Ok(None),
        }
    }

    fn provider_name(&self) -> &'static str {
        "generic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwards_extra_headers_and_bearer_auth() {
        let mut extra = BTreeMap::new();
        extra.insert(
            "HTTP-Referer".to_string(),
            "https://gwenland.dev".to_string(),
        );
        extra.insert("X-Title".to_string(), "GwenLand IDE".to_string());

        let headers = build_headers("test-key-123", &extra).unwrap();

        assert_eq!(headers[AUTHORIZATION], "Bearer test-key-123");
        assert_eq!(headers[CONTENT_TYPE], "application/json");
        assert_eq!(headers["HTTP-Referer"], "https://gwenland.dev");
        assert_eq!(headers["X-Title"], "GwenLand IDE");
    }

    #[test]
    fn rejects_invalid_header_value() {
        let mut extra = BTreeMap::new();
        // A newline is not a valid header value.
        extra.insert("X-Bad".to_string(), "line1\nline2".to_string());
        assert!(matches!(
            build_headers("k", &extra),
            Err(AiError::ProviderError(_))
        ));
    }

    #[test]
    fn endpoint_normalizes_trailing_slash() {
        let a = GenericAdapter {
            id: "generic-x".into(),
            base_url: "https://api.example.com/v1/".into(),
            default_model: "m".into(),
            extra_headers: BTreeMap::new(),
        };
        assert_eq!(
            a.endpoint("chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }
}
