//! OS keychain wrapper (Requirement 9).
//!
//! The ONLY place API keys are stored. Backed by the OS secret store via the
//! `keyring` crate: Windows Credential Manager, macOS Keychain, or Linux Secret
//! Service. Keys are NEVER written to `settings.toml`, JSONL, the manifest,
//! logs, events, or frontend stores. If the OS store is unavailable we return
//! [`AiError::KeychainError`] and never fall back to plaintext (Req 9.8).
//!
//! Account ids are the provider ids: `anthropic`, `openai`, `gemini`, and each
//! generic provider id (Requirement 9.3).

use keyring::Entry;

use crate::ai::error::AiError;

/// Keychain service name shared by every account (Requirement 9.2).
pub const SERVICE_NAME: &str = "gwenland-ide";

fn entry(provider: &str) -> Result<Entry, AiError> {
    Entry::new(SERVICE_NAME, provider).map_err(|e| AiError::KeychainError(e.to_string()))
}

/// Store (or replace) the key for `provider`. Surrounding whitespace is trimmed
/// and empty keys are rejected (Requirement 9.5).
pub fn set_api_key(provider: &str, api_key: &str) -> Result<(), AiError> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return Err(AiError::KeychainError("API key must not be empty".into()));
    }
    entry(provider)?
        .set_password(trimmed)
        .map_err(|e| AiError::KeychainError(e.to_string()))
}

/// Fetch the stored key. Missing keys map to [`AiError::KeyNotSet`] (Req 9 /
/// used by adapters at send time).
pub fn get_api_key(provider: &str) -> Result<String, AiError> {
    match entry(provider)?.get_password() {
        Ok(p) => Ok(p),
        Err(keyring::Error::NoEntry) => Err(AiError::KeyNotSet),
        Err(e) => Err(AiError::KeychainError(e.to_string())),
    }
}

/// Delete the key. Idempotent: deleting an absent key is success (Req 9.7).
pub fn delete_api_key(provider: &str) -> Result<(), AiError> {
    match entry(provider)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AiError::KeychainError(e.to_string())),
    }
}

/// Report only whether a key is stored — never the value (Requirement 9.6).
pub fn has_api_key(provider: &str) -> Result<bool, AiError> {
    match get_api_key(provider) {
        Ok(_) => Ok(true),
        Err(AiError::KeyNotSet) => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full lifecycle against the real OS keychain. Gated: if the keychain is
    /// unavailable (headless CI, no Secret Service), we skip cleanly rather than
    /// fail (Requirement 20.2). Uses a unique account so we never touch real
    /// user keys, and cleans up afterward.
    #[test]
    fn set_get_delete_has_round_trip() {
        let provider = format!("__gwenland_test_{}", uuid::Uuid::new_v4());

        // Probe availability. A KeychainError here means no usable store -> skip.
        match set_api_key(&provider, "  secret-value  ") {
            Ok(()) => {}
            Err(AiError::KeychainError(_)) => {
                eprintln!("skipping keychain test: OS keychain unavailable");
                return;
            }
            Err(e) => panic!("unexpected error probing keychain: {e:?}"),
        }

        // Whitespace was trimmed on store.
        assert_eq!(get_api_key(&provider).unwrap(), "secret-value");
        assert!(has_api_key(&provider).unwrap());

        // Delete, then it's absent; deleting again is still Ok (idempotent).
        delete_api_key(&provider).unwrap();
        assert!(!has_api_key(&provider).unwrap());
        assert!(matches!(get_api_key(&provider), Err(AiError::KeyNotSet)));
        delete_api_key(&provider).unwrap();
    }

    #[test]
    fn empty_key_is_rejected() {
        assert!(matches!(
            set_api_key("anthropic", "   "),
            Err(AiError::KeychainError(_))
        ));
    }
}
