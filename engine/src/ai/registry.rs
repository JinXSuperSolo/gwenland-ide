//! Provider registry (Requirement 3.1-3.3).
//!
//! Resolves a provider id to a boxed `dyn AiProvider` the command layer can use
//! without caring which concrete adapter it got. Native ids (`anthropic`,
//! `openai`, `gemini`) map to their adapters; ids beginning with `generic-` are
//! looked up in the user's `AiSettings.generic_providers`. Anything else is an
//! `AiError::ProviderError`.

use crate::ai::anthropic::AnthropicAdapter;
use crate::ai::error::AiError;
use crate::ai::gemini::GeminiAdapter;
use crate::ai::generic::GenericAdapter;
use crate::ai::openai::OpenAiAdapter;
use crate::ai::provider::AiProvider;
use crate::system::settings::AiSettings;

/// Build a provider instance for `provider_id`. `settings` supplies generic
/// provider configuration.
pub fn resolve_provider(
    provider_id: &str,
    settings: &AiSettings,
) -> Result<Box<dyn AiProvider>, AiError> {
    match provider_id {
        "anthropic" => Ok(Box::new(AnthropicAdapter::new())),
        "openai" => Ok(Box::new(OpenAiAdapter::new())),
        "gemini" => Ok(Box::new(GeminiAdapter::new())),
        id if id.starts_with("generic-") => {
            let cfg = settings
                .generic_providers
                .iter()
                .find(|g| g.id == id)
                .ok_or_else(|| {
                    AiError::ProviderError(format!("unknown generic provider id: {id}"))
                })?;
            Ok(Box::new(GenericAdapter::from_setting(cfg)))
        }
        other => Err(AiError::ProviderError(format!(
            "unknown provider id: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::settings::GenericProviderSetting;

    fn settings_with_generic() -> AiSettings {
        AiSettings {
            generic_providers: vec![GenericProviderSetting {
                id: "generic-groq".into(),
                display_name: "Groq".into(),
                base_url: "https://api.groq.com/openai/v1".into(),
                default_model: "llama-3.1-70b-versatile".into(),
                extra_headers: Default::default(),
            }],
            ..AiSettings::default()
        }
    }

    #[test]
    fn resolves_native_providers() {
        let s = AiSettings::default();
        assert_eq!(
            resolve_provider("anthropic", &s).unwrap().provider_name(),
            "anthropic"
        );
        assert_eq!(
            resolve_provider("openai", &s).unwrap().provider_name(),
            "openai"
        );
        assert_eq!(
            resolve_provider("gemini", &s).unwrap().provider_name(),
            "gemini"
        );
    }

    #[test]
    fn resolves_configured_generic_provider() {
        let s = settings_with_generic();
        let p = resolve_provider("generic-groq", &s).unwrap();
        assert_eq!(p.provider_name(), "generic");
    }

    #[test]
    fn rejects_unknown_generic_id() {
        let s = settings_with_generic();
        assert!(matches!(
            resolve_provider("generic-missing", &s),
            Err(AiError::ProviderError(_))
        ));
    }

    #[test]
    fn rejects_unknown_provider() {
        let s = AiSettings::default();
        assert!(matches!(
            resolve_provider("totally-bogus", &s),
            Err(AiError::ProviderError(_))
        ));
    }
}
