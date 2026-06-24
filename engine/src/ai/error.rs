//! Normalized AI error type.
//!
//! Every provider adapter maps its transport- and provider-specific failures
//! into one of these variants before crossing the `AiProvider` trait boundary
//! (Requirement 19.6). This keeps the Tauri/UI layers provider-agnostic and,
//! critically, guarantees no API key, authorization header, or raw request
//! payload can leak through an error (Requirement 19.9): construct variants
//! only from sanitized strings.
//!
//! The serde representation is *adjacently tagged* (`{ "kind": ..., "data": ... }`)
//! so the UI can discriminate on `kind` and the type round-trips through serde
//! for every variant, including tuple and struct shapes.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum AiError {
    /// No API key is stored in the OS keychain for this provider.
    #[error("no API key is set for this provider")]
    KeyNotSet,

    /// The provider rejected the key (HTTP 401/403).
    #[error("the API key is invalid or unauthorized")]
    InvalidKey,

    /// The provider is rate-limiting us (HTTP 429). `retry_after` carries the
    /// parsed `Retry-After` header value in seconds when present.
    #[error("rate limited by the provider")]
    RateLimit { retry_after: Option<u64> },

    /// The request exceeded the model's context window (detectable provider
    /// signal, e.g. HTTP 413 or a context-length error body).
    #[error("the request exceeded the model context length")]
    ContextLengthExceeded,

    /// Transport-level failure (DNS, TLS, connection reset, timeout).
    #[error("network error: {0}")]
    Network(String),

    /// Any other provider-side failure (5xx, malformed response, unknown id).
    /// The string is a sanitized, key-free message.
    #[error("provider error: {0}")]
    ProviderError(String),

    /// The stream was cancelled by the user. The UI suppresses the red banner
    /// for this variant (Requirement 11.10 / 19.8).
    #[error("the request was cancelled")]
    Cancelled,

    /// The OS keychain was unavailable or refused the operation. The engine
    /// never falls back to plaintext storage (Requirement 9.8).
    #[error("keychain error: {0}")]
    KeychainError(String),

    /// Conversation JSONL / manifest persistence failure.
    #[error("storage error: {0}")]
    StorageError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant round-trips through serde (Requirement 20.6).
    #[test]
    fn aierror_serde_round_trip_all_variants() {
        let variants = [
            AiError::KeyNotSet,
            AiError::InvalidKey,
            AiError::RateLimit { retry_after: None },
            AiError::RateLimit {
                retry_after: Some(30),
            },
            AiError::ContextLengthExceeded,
            AiError::Network("connection reset".into()),
            AiError::ProviderError("upstream 503".into()),
            AiError::Cancelled,
            AiError::KeychainError("secret service unavailable".into()),
            AiError::StorageError("disk full".into()),
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let back: AiError = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back, "round-trip mismatch for {json}");
        }
    }

    #[test]
    fn aierror_serializes_with_kind_tag() {
        let json = serde_json::to_value(AiError::KeyNotSet).unwrap();
        assert_eq!(json["kind"], "key_not_set");

        let json = serde_json::to_value(AiError::RateLimit {
            retry_after: Some(5),
        })
        .unwrap();
        assert_eq!(json["kind"], "rate_limit");
        assert_eq!(json["data"]["retry_after"], 5);
    }
}
