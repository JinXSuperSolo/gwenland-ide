//! Provider-neutral HTTP error mapping (Requirement 19.2-19.5).
//!
//! Every adapter funnels its initial non-2xx response through [`map_status`] so
//! the same status code yields the same normalized [`AiError`] everywhere:
//! 401/403 → `InvalidKey`, 429 → `RateLimit` (with parsed `Retry-After`),
//! 413/context bodies → `ContextLengthExceeded`, 5xx → `ProviderError`. The
//! `provider` label is only used to build a key-free diagnostic string.

use crate::ai::error::AiError;

/// Parse the `Retry-After` header as whole seconds, if present and numeric.
pub fn retry_after_secs(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
}

/// Map a non-success status + response body to a normalized error. `body` is a
/// best-effort text snapshot used only to sniff context-length failures; it is
/// never echoed verbatim into the error (avoids leaking request payloads).
pub fn map_status(
    status: reqwest::StatusCode,
    retry_after: Option<u64>,
    body: &str,
    provider: &str,
) -> AiError {
    match status.as_u16() {
        401 | 403 => AiError::InvalidKey,
        429 => AiError::RateLimit { retry_after },
        413 => AiError::ContextLengthExceeded,
        s if s >= 500 => AiError::ProviderError(format!("{provider} HTTP {s}")),
        s if looks_like_context_error(body) => {
            let _ = s;
            AiError::ContextLengthExceeded
        }
        s => AiError::ProviderError(format!("{provider} HTTP {s}")),
    }
}

/// Heuristic for context-length / oversized-request errors that arrive as 400s.
pub fn looks_like_context_error(body: &str) -> bool {
    let lower = body.to_lowercase();
    lower.contains("context length")
        || lower.contains("context_length")
        || lower.contains("maximum context")
        || lower.contains("too long")
        || lower.contains("too large")
        || lower.contains("reduce the length")
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn maps_auth_failures_to_invalid_key() {
        assert_eq!(
            map_status(StatusCode::UNAUTHORIZED, None, "", "X"),
            AiError::InvalidKey
        );
        assert_eq!(
            map_status(StatusCode::FORBIDDEN, None, "", "X"),
            AiError::InvalidKey
        );
    }

    #[test]
    fn maps_429_to_rate_limit_with_retry_after() {
        assert_eq!(
            map_status(StatusCode::TOO_MANY_REQUESTS, Some(7), "", "X"),
            AiError::RateLimit {
                retry_after: Some(7)
            }
        );
    }

    #[test]
    fn maps_413_and_context_body_to_context_length() {
        assert_eq!(
            map_status(StatusCode::PAYLOAD_TOO_LARGE, None, "", "X"),
            AiError::ContextLengthExceeded
        );
        assert_eq!(
            map_status(
                StatusCode::BAD_REQUEST,
                None,
                "This model's maximum context length is 8192 tokens",
                "X"
            ),
            AiError::ContextLengthExceeded
        );
    }

    #[test]
    fn maps_5xx_and_other_4xx_to_provider_error() {
        assert!(matches!(
            map_status(StatusCode::INTERNAL_SERVER_ERROR, None, "", "X"),
            AiError::ProviderError(_)
        ));
        assert!(matches!(
            map_status(StatusCode::NOT_FOUND, None, "nope", "X"),
            AiError::ProviderError(_)
        ));
    }

    #[test]
    fn retry_after_parses_numeric_header() {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(reqwest::header::RETRY_AFTER, "42".parse().unwrap());
        assert_eq!(retry_after_secs(&h), Some(42));
        assert_eq!(retry_after_secs(&reqwest::header::HeaderMap::new()), None);
    }
}
