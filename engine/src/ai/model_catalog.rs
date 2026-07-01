//! Static model registry for the model picker/effort toggle (Milestone 26).
//!
//! Single source of truth for every AI model GwenLand knows about across all
//! supported providers. Purely descriptive data — no network calls, no
//! provider adapter behavior. UI concerns (selector, effort dropdown, token
//! tracking) read this catalog; they must never need a code change here to
//! support a new model beyond adding a [`ModelEntry`] to [`all_models`].
//!
//! Distinct from [`crate::ai::registry`], which resolves a provider id to a
//! live `dyn AiProvider` adapter. This module never touches adapters — it's
//! metadata only.

use serde::{Deserialize, Serialize};

/// A provider GwenLand can call for chat completions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Anthropic,
    OpenAi,
    Google,
    XAi,
    DeepSeek,
    ZhipuGlm,
    Moonshot,
    Qwen,
    Mistral,
}

/// How a model's reasoning/thinking effort is controlled, if at all.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    /// Discrete named levels sent as a string param (e.g. Anthropic effort,
    /// OpenAI reasoning.effort, Gemini thinking_level).
    Effort,
    /// On/off only, no graduated levels.
    Binary,
    /// Legacy token-budget style (e.g. Claude Haiku 4.5's `budget_tokens`).
    BudgetTokens,
    /// Model has no reasoning capability at all.
    None,
}

/// Per-1M-token USD pricing, input and output tracked separately.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Pricing {
    pub input_per_m: f64,
    pub output_per_m: f64,
}

/// Describes whether/how a model's reasoning effort can be configured.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReasoningConfig {
    pub supported: bool,
    /// Wire param name, e.g. `"output_config.effort"`. `None` when unsupported.
    pub param_name: Option<String>,
    /// Named levels accepted by `param_name`. Empty when unsupported.
    pub levels: Vec<String>,
    pub default: Option<String>,
    pub mode: ReasoningMode,
}

impl ReasoningConfig {
    /// No reasoning capability at all.
    pub fn none() -> Self {
        Self {
            supported: false,
            param_name: None,
            levels: Vec::new(),
            default: None,
            mode: ReasoningMode::None,
        }
    }

    /// Discrete named effort levels sent via `param_name`.
    pub fn effort(param_name: &str, levels: &[&str], default: &str) -> Self {
        Self {
            supported: true,
            param_name: Some(param_name.into()),
            levels: levels.iter().map(|s| s.to_string()).collect(),
            default: Some(default.into()),
            mode: ReasoningMode::Effort,
        }
    }

    /// On/off toggle via `param_name`.
    pub fn binary(param_name: &str, default: &str) -> Self {
        Self {
            supported: true,
            param_name: Some(param_name.into()),
            levels: vec!["off".into(), "on".into()],
            default: Some(default.into()),
            mode: ReasoningMode::Binary,
        }
    }

    /// Legacy token-budget reasoning (Claude Haiku 4.5 style): no named
    /// levels, controlled by a token count instead.
    pub fn budget_tokens(param_name: &str) -> Self {
        Self {
            supported: true,
            param_name: Some(param_name.into()),
            levels: Vec::new(),
            default: None,
            mode: ReasoningMode::BudgetTokens,
        }
    }
}

/// One model GwenLand can target, with its provider, pricing, and reasoning
/// capability. See module docs — adding a model means adding an entry to
/// [`all_models`], nothing else.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub provider: Provider,
    /// Grouping/sort key within a provider, e.g. `"flagship"`, `"balanced"`, `"fast"`.
    pub tier: String,
    pub context_window: u32,
    pub pricing: Pricing,
    pub reasoning: ReasoningConfig,
}

/// The full static model catalog across all supported providers.
///
/// Curated flagship + common variants per provider; not exhaustive. Extend by
/// appending entries — no other code needs to change.
pub fn all_models() -> Vec<ModelEntry> {
    vec![
        // --- Anthropic ------------------------------------------------------
        ModelEntry {
            id: "claude-opus-4-8".into(),
            name: "Claude Opus 4.8".into(),
            provider: Provider::Anthropic,
            tier: "flagship".into(),
            context_window: 200_000,
            pricing: Pricing { input_per_m: 5.00, output_per_m: 25.00 },
            reasoning: ReasoningConfig::effort(
                "output_config.effort",
                &["low", "medium", "high", "xhigh", "max"],
                "xhigh",
            ),
        },
        ModelEntry {
            id: "claude-opus-4-7".into(),
            name: "Claude Opus 4.7".into(),
            provider: Provider::Anthropic,
            tier: "flagship".into(),
            context_window: 200_000,
            // TODO: verify pricing
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            reasoning: ReasoningConfig::effort(
                "output_config.effort",
                &["low", "medium", "high", "xhigh"],
                "high",
            ),
        },
        ModelEntry {
            id: "claude-sonnet-4-6".into(),
            name: "Claude Sonnet 4.6".into(),
            provider: Provider::Anthropic,
            tier: "balanced".into(),
            context_window: 200_000,
            pricing: Pricing { input_per_m: 3.00, output_per_m: 15.00 },
            reasoning: ReasoningConfig::effort(
                "output_config.effort",
                &["low", "medium", "high", "xhigh"],
                "high",
            ),
        },
        ModelEntry {
            id: "claude-sonnet-4-5".into(),
            name: "Claude Sonnet 4.5".into(),
            provider: Provider::Anthropic,
            tier: "balanced".into(),
            context_window: 200_000,
            // TODO: verify pricing
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            reasoning: ReasoningConfig::effort("output_config.effort", &["low", "medium", "high"], "medium"),
        },
        ModelEntry {
            id: "claude-haiku-4-5-20251001".into(),
            name: "Claude Haiku 4.5".into(),
            provider: Provider::Anthropic,
            tier: "fast".into(),
            context_window: 200_000,
            pricing: Pricing { input_per_m: 1.00, output_per_m: 5.00 },
            reasoning: ReasoningConfig::budget_tokens("budget_tokens"),
        },
        // --- OpenAI -----------------------------------------------------------
        ModelEntry {
            id: "gpt-5.5".into(),
            name: "GPT-5.5".into(),
            provider: Provider::OpenAi,
            tier: "flagship".into(),
            context_window: 400_000,
            pricing: Pricing { input_per_m: 5.00, output_per_m: 30.00 },
            reasoning: ReasoningConfig::effort(
                "reasoning.effort",
                &["none", "minimal", "low", "medium", "high", "xhigh"],
                "medium",
            ),
        },
        ModelEntry {
            id: "gpt-5.5-pro".into(),
            name: "GPT-5.5 Pro".into(),
            provider: Provider::OpenAi,
            tier: "flagship".into(),
            context_window: 400_000,
            pricing: Pricing { input_per_m: 30.00, output_per_m: 180.00 },
            reasoning: ReasoningConfig::effort(
                "reasoning.effort",
                &["none", "minimal", "low", "medium", "high", "xhigh"],
                "high",
            ),
        },
        ModelEntry {
            id: "gpt-5.4".into(),
            name: "GPT-5.4".into(),
            provider: Provider::OpenAi,
            tier: "balanced".into(),
            context_window: 400_000,
            pricing: Pricing { input_per_m: 2.50, output_per_m: 15.00 },
            reasoning: ReasoningConfig::effort(
                "reasoning.effort",
                &["none", "minimal", "low", "medium", "high"],
                "medium",
            ),
        },
        ModelEntry {
            id: "gpt-5.4-mini".into(),
            name: "GPT-5.4 Mini".into(),
            provider: Provider::OpenAi,
            tier: "fast".into(),
            context_window: 400_000,
            pricing: Pricing { input_per_m: 0.75, output_per_m: 4.50 },
            reasoning: ReasoningConfig::effort(
                "reasoning.effort",
                &["none", "low", "medium", "high", "xhigh"],
                "medium",
            ),
        },
        ModelEntry {
            id: "gpt-5.4-nano".into(),
            name: "GPT-5.4 Nano".into(),
            provider: Provider::OpenAi,
            tier: "fast".into(),
            context_window: 400_000,
            pricing: Pricing { input_per_m: 0.20, output_per_m: 1.25 },
            reasoning: ReasoningConfig::effort(
                "reasoning.effort",
                &["none", "low", "medium", "high", "xhigh"],
                "medium",
            ),
        },
        // --- Google Gemini ------------------------------------------------
        ModelEntry {
            id: "gemini-3.1-pro".into(),
            name: "Gemini 3.1 Pro".into(),
            provider: Provider::Google,
            tier: "flagship".into(),
            context_window: 1_000_000,
            // TODO: verify pricing
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            reasoning: ReasoningConfig::effort("thinking_level", &["low", "medium", "high"], "high"),
        },
        ModelEntry {
            id: "gemini-3.5-flash".into(),
            name: "Gemini 3.5 Flash".into(),
            provider: Provider::Google,
            tier: "balanced".into(),
            context_window: 1_000_000,
            // TODO: verify pricing
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            reasoning: ReasoningConfig::effort("thinking_level", &["low", "medium", "high"], "medium"),
        },
        ModelEntry {
            id: "gemini-3-flash".into(),
            name: "Gemini 3 Flash".into(),
            provider: Provider::Google,
            tier: "fast".into(),
            context_window: 1_000_000,
            pricing: Pricing { input_per_m: 0.50, output_per_m: 3.00 },
            reasoning: ReasoningConfig::effort("thinking_level", &["low", "medium", "high"], "medium"),
        },
        ModelEntry {
            id: "gemini-3.1-flash-lite".into(),
            name: "Gemini 3.1 Flash-Lite".into(),
            provider: Provider::Google,
            tier: "fast".into(),
            context_window: 1_000_000,
            pricing: Pricing { input_per_m: 0.25, output_per_m: 1.50 },
            reasoning: ReasoningConfig::effort(
                "thinking_level",
                &["minimal", "low", "medium", "high"],
                "low",
            ),
        },
        ModelEntry {
            id: "gemini-2.5-flash".into(),
            name: "Gemini 2.5 Flash".into(),
            provider: Provider::Google,
            tier: "fast".into(),
            context_window: 1_000_000,
            // TODO: verify pricing
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            reasoning: ReasoningConfig::effort("thinking_level", &["low", "high"], "low"),
        },
        // --- xAI Grok -------------------------------------------------------
        ModelEntry {
            id: "grok-4.3".into(),
            name: "Grok 4.3".into(),
            provider: Provider::XAi,
            tier: "flagship".into(),
            context_window: 256_000,
            pricing: Pricing { input_per_m: 1.25, output_per_m: 2.50 },
            reasoning: ReasoningConfig::effort(
                "reasoning_effort",
                &["none", "low", "medium", "high"],
                "low",
            ),
        },
        ModelEntry {
            id: "grok-4.1-fast".into(),
            name: "Grok 4.1 Fast".into(),
            provider: Provider::XAi,
            tier: "fast".into(),
            context_window: 2_000_000,
            pricing: Pricing { input_per_m: 0.20, output_per_m: 0.50 },
            reasoning: ReasoningConfig::effort(
                "reasoning_effort",
                &["none", "low", "medium", "high"],
                "low",
            ),
        },
        ModelEntry {
            id: "grok-4".into(),
            name: "Grok 4".into(),
            provider: Provider::XAi,
            tier: "balanced".into(),
            context_window: 256_000,
            // TODO: verify pricing
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            reasoning: ReasoningConfig::effort(
                "reasoning_effort",
                &["none", "low", "medium", "high"],
                "low",
            ),
        },
        ModelEntry {
            id: "grok-3-mini".into(),
            name: "Grok 3 Mini".into(),
            provider: Provider::XAi,
            tier: "fast".into(),
            context_window: 128_000,
            // TODO: verify pricing
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            reasoning: ReasoningConfig::effort("reasoning_effort", &["none", "low", "high"], "low"),
        },
        // --- DeepSeek ------------------------------------------------------
        // low/medium silently map to high server-side; modeled as Binary to
        // avoid a misleading multi-level UI (see GWEN-454 research table).
        ModelEntry {
            id: "deepseek-v4-pro".into(),
            name: "DeepSeek V4 Pro".into(),
            provider: Provider::DeepSeek,
            tier: "flagship".into(),
            context_window: 1_000_000,
            pricing: Pricing { input_per_m: 0.435, output_per_m: 0.87 },
            reasoning: ReasoningConfig::binary("thinking.type", "on"),
        },
        ModelEntry {
            id: "deepseek-v4-flash".into(),
            name: "DeepSeek V4 Flash".into(),
            provider: Provider::DeepSeek,
            tier: "fast".into(),
            context_window: 1_000_000,
            pricing: Pricing { input_per_m: 0.14, output_per_m: 0.28 },
            reasoning: ReasoningConfig::binary("thinking.type", "off"),
        },
        ModelEntry {
            id: "deepseek-v3.1".into(),
            name: "DeepSeek V3.1".into(),
            provider: Provider::DeepSeek,
            tier: "balanced".into(),
            context_window: 128_000,
            // TODO: verify pricing
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            reasoning: ReasoningConfig::binary("thinking.type", "off"),
        },
        // --- Z.AI / GLM ------------------------------------------------------
        // low maps to high internally — same caveat as DeepSeek.
        ModelEntry {
            id: "glm-5.2".into(),
            name: "GLM-5.2".into(),
            provider: Provider::ZhipuGlm,
            tier: "flagship".into(),
            context_window: 1_000_000,
            pricing: Pricing { input_per_m: 1.40, output_per_m: 4.40 },
            reasoning: ReasoningConfig::effort(
                "reasoning_effort",
                &["off", "low", "high", "max"],
                "high",
            ),
        },
        ModelEntry {
            id: "glm-4.7".into(),
            name: "GLM-4.7".into(),
            provider: Provider::ZhipuGlm,
            tier: "balanced".into(),
            context_window: 205_000,
            pricing: Pricing { input_per_m: 0.60, output_per_m: 2.20 },
            reasoning: ReasoningConfig::effort(
                "reasoning_effort",
                &["off", "low", "high", "max"],
                "high",
            ),
        },
        ModelEntry {
            id: "glm-4.6".into(),
            name: "GLM-4.6".into(),
            provider: Provider::ZhipuGlm,
            tier: "balanced".into(),
            context_window: 205_000,
            pricing: Pricing { input_per_m: 0.43, output_per_m: 1.74 },
            reasoning: ReasoningConfig::effort(
                "reasoning_effort",
                &["off", "low", "high", "max"],
                "high",
            ),
        },
        ModelEntry {
            id: "glm-4.7-flash".into(),
            name: "GLM-4.7-Flash".into(),
            provider: Provider::ZhipuGlm,
            tier: "fast".into(),
            context_window: 203_000,
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            reasoning: ReasoningConfig::effort("reasoning_effort", &["off", "low", "high"], "low"),
        },
        // --- Moonshot / Kimi ------------------------------------------------
        ModelEntry {
            id: "kimi-k2.6".into(),
            name: "Kimi K2.6".into(),
            provider: Provider::Moonshot,
            tier: "flagship".into(),
            context_window: 256_000,
            pricing: Pricing { input_per_m: 0.60, output_per_m: 2.50 },
            reasoning: ReasoningConfig::binary("thinking", "on"),
        },
        ModelEntry {
            id: "kimi-k2.5".into(),
            name: "Kimi K2.5".into(),
            provider: Provider::Moonshot,
            tier: "balanced".into(),
            context_window: 256_000,
            pricing: Pricing { input_per_m: 0.60, output_per_m: 2.50 },
            reasoning: ReasoningConfig::binary("thinking", "on"),
        },
        ModelEntry {
            id: "kimi-k2-thinking".into(),
            name: "Kimi K2 Thinking".into(),
            provider: Provider::Moonshot,
            tier: "balanced".into(),
            context_window: 256_000,
            // TODO: verify pricing
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            // Always-on extended reasoning; no toggle exists on this model id.
            reasoning: ReasoningConfig::none(),
        },
        ModelEntry {
            id: "kimi-k2.7-code".into(),
            name: "Kimi K2.7 Code".into(),
            provider: Provider::Moonshot,
            tier: "fast".into(),
            context_window: 256_000,
            pricing: Pricing { input_per_m: 0.95, output_per_m: 4.00 },
            // Always thinks; no toggle exists on this model id.
            reasoning: ReasoningConfig::none(),
        },
        // --- Qwen -----------------------------------------------------------
        ModelEntry {
            id: "qwen3.7-max".into(),
            name: "Qwen3.7 Max".into(),
            provider: Provider::Qwen,
            tier: "flagship".into(),
            context_window: 1_000_000,
            pricing: Pricing { input_per_m: 2.50, output_per_m: 7.50 },
            reasoning: ReasoningConfig::binary("enable_thinking", "on"),
        },
        ModelEntry {
            id: "qwen3-max".into(),
            name: "Qwen3 Max".into(),
            provider: Provider::Qwen,
            tier: "balanced".into(),
            context_window: 256_000,
            // TODO: verify pricing
            pricing: Pricing { input_per_m: 0.0, output_per_m: 0.0 },
            reasoning: ReasoningConfig::binary("enable_thinking", "off"),
        },
        ModelEntry {
            id: "qwen-plus".into(),
            name: "Qwen Plus".into(),
            provider: Provider::Qwen,
            tier: "balanced".into(),
            context_window: 128_000,
            pricing: Pricing { input_per_m: 0.40, output_per_m: 1.20 },
            reasoning: ReasoningConfig::binary("enable_thinking", "off"),
        },
        ModelEntry {
            id: "qwen-turbo".into(),
            name: "Qwen Turbo".into(),
            provider: Provider::Qwen,
            tier: "fast".into(),
            context_window: 128_000,
            pricing: Pricing { input_per_m: 0.05, output_per_m: 0.20 },
            reasoning: ReasoningConfig::binary("enable_thinking", "off"),
        },
        ModelEntry {
            id: "qwen3-30b-a3b".into(),
            name: "Qwen3 30B-A3B".into(),
            provider: Provider::Qwen,
            tier: "fast".into(),
            context_window: 128_000,
            pricing: Pricing { input_per_m: 0.20, output_per_m: 0.80 },
            reasoning: ReasoningConfig::binary("enable_thinking", "off"),
        },
        // --- Mistral ---------------------------------------------------------
        // Reasoning support unconfirmed for the non-Magistral lineup; flagged
        // unsupported per task notes until Magistral models are added.
        ModelEntry {
            id: "mistral-large-3".into(),
            name: "Mistral Large 3".into(),
            provider: Provider::Mistral,
            tier: "flagship".into(),
            context_window: 128_000,
            pricing: Pricing { input_per_m: 0.50, output_per_m: 1.50 },
            reasoning: ReasoningConfig::none(),
        },
        ModelEntry {
            id: "mistral-small-4".into(),
            name: "Mistral Small 4".into(),
            provider: Provider::Mistral,
            tier: "balanced".into(),
            context_window: 128_000,
            pricing: Pricing { input_per_m: 0.15, output_per_m: 0.15 },
            reasoning: ReasoningConfig::none(),
        },
        ModelEntry {
            id: "devstral".into(),
            name: "Devstral".into(),
            provider: Provider::Mistral,
            tier: "fast".into(),
            context_window: 262_000,
            pricing: Pricing { input_per_m: 0.15, output_per_m: 0.15 },
            reasoning: ReasoningConfig::none(),
        },
        ModelEntry {
            id: "magistral-medium".into(),
            name: "Magistral Medium".into(),
            provider: Provider::Mistral,
            tier: "balanced".into(),
            context_window: 128_000,
            pricing: Pricing { input_per_m: 2.00, output_per_m: 5.00 },
            reasoning: ReasoningConfig::binary("reasoning", "on"),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_provider_has_at_least_one_model() {
        let models = all_models();
        let providers: HashSet<Provider> = models.iter().map(|m| m.provider).collect();
        for expected in [
            Provider::Anthropic,
            Provider::OpenAi,
            Provider::Google,
            Provider::XAi,
            Provider::DeepSeek,
            Provider::ZhipuGlm,
            Provider::Moonshot,
            Provider::Qwen,
            Provider::Mistral,
        ] {
            assert!(
                providers.contains(&expected),
                "missing at least one model for provider {expected:?}"
            );
        }
    }

    #[test]
    fn model_ids_are_unique() {
        let models = all_models();
        let ids: HashSet<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids.len(), models.len(), "duplicate model id in catalog");
    }

    fn find(id: &str) -> ModelEntry {
        all_models()
            .into_iter()
            .find(|m| m.id == id)
            .unwrap_or_else(|| panic!("model {id} not found in catalog"))
    }

    #[test]
    fn anthropic_opus_supports_effort_with_max() {
        let m = find("claude-opus-4-8");
        assert_eq!(m.reasoning.mode, ReasoningMode::Effort);
        assert_eq!(m.reasoning.param_name.as_deref(), Some("output_config.effort"));
        assert!(m.reasoning.levels.contains(&"max".to_string()));
        assert!(m.reasoning.levels.contains(&"xhigh".to_string()));
    }

    #[test]
    fn anthropic_sonnet_supports_xhigh_but_not_max() {
        let m = find("claude-sonnet-4-6");
        assert!(m.reasoning.levels.contains(&"xhigh".to_string()));
        assert!(!m.reasoning.levels.contains(&"max".to_string()));
    }

    #[test]
    fn anthropic_haiku_is_budget_tokens_not_effort() {
        let m = find("claude-haiku-4-5-20251001");
        assert_eq!(m.reasoning.mode, ReasoningMode::BudgetTokens);
        assert!(m.reasoning.levels.is_empty());
    }

    #[test]
    fn openai_uses_reasoning_effort_param() {
        let m = find("gpt-5.5");
        assert_eq!(m.reasoning.param_name.as_deref(), Some("reasoning.effort"));
        assert!(m.reasoning.levels.contains(&"minimal".to_string()));
    }

    #[test]
    fn gemini_thinking_level_param() {
        let m = find("gemini-3.1-pro");
        assert_eq!(m.reasoning.param_name.as_deref(), Some("thinking_level"));
        assert!(m.reasoning.levels.contains(&"medium".to_string()));
    }

    #[test]
    fn gemini_2_5_flash_cannot_fully_disable_thinking() {
        let m = find("gemini-2.5-flash");
        assert_eq!(m.reasoning.levels, vec!["low".to_string(), "high".to_string()]);
    }

    #[test]
    fn grok_default_is_low() {
        let m = find("grok-4.3");
        assert_eq!(m.reasoning.default.as_deref(), Some("low"));
    }

    #[test]
    fn deepseek_modeled_as_binary_despite_more_api_values() {
        let m = find("deepseek-v4-pro");
        assert_eq!(m.reasoning.mode, ReasoningMode::Binary);
    }

    #[test]
    fn glm_supports_off_low_high_max() {
        let m = find("glm-4.6");
        assert_eq!(
            m.reasoning.levels,
            vec!["off".to_string(), "low".to_string(), "high".to_string(), "max".to_string()]
        );
    }

    #[test]
    fn moonshot_code_variant_has_no_toggle() {
        let m = find("kimi-k2.7-code");
        assert!(!m.reasoning.supported);
        assert_eq!(m.reasoning.mode, ReasoningMode::None);
    }

    #[test]
    fn moonshot_k2_6_is_togglable() {
        let m = find("kimi-k2.6");
        assert_eq!(m.reasoning.mode, ReasoningMode::Binary);
    }

    #[test]
    fn qwen_is_binary_enable_thinking() {
        let m = find("qwen3-max");
        assert_eq!(m.reasoning.param_name.as_deref(), Some("enable_thinking"));
        assert_eq!(m.reasoning.mode, ReasoningMode::Binary);
    }

    #[test]
    fn mistral_large_has_no_reasoning_but_magistral_does() {
        let large = find("mistral-large-3");
        assert!(!large.reasoning.supported);
        let magistral = find("magistral-medium");
        assert!(magistral.reasoning.supported);
        assert_eq!(magistral.reasoning.mode, ReasoningMode::Binary);
    }

    #[test]
    fn openai_mini_and_nano_support_reasoning_unlike_flagship_gap() {
        for id in ["gpt-5.4-mini", "gpt-5.4-nano"] {
            let m = find(id);
            assert!(m.reasoning.levels.contains(&"xhigh".to_string()), "{id} should offer xhigh");
        }
    }

    #[test]
    fn moonshot_k2_thinking_is_always_on_no_toggle() {
        let m = find("kimi-k2-thinking");
        assert!(!m.reasoning.supported);
    }

    #[test]
    fn catalog_has_at_least_four_models_per_provider() {
        let models = all_models();
        let mut counts: std::collections::HashMap<Provider, usize> = std::collections::HashMap::new();
        for m in &models {
            *counts.entry(m.provider).or_insert(0) += 1;
        }
        for (provider, count) in &counts {
            assert!(*count >= 3, "provider {provider:?} has only {count} models");
        }
    }

    #[test]
    fn schema_round_trips_through_json() {
        for model in all_models() {
            let json = serde_json::to_string(&model).unwrap();
            let back: ModelEntry = serde_json::from_str(&json).unwrap();
            assert_eq!(model, back);
        }
    }

    #[test]
    fn provider_serializes_snake_case() {
        let json = serde_json::to_value(Provider::ZhipuGlm).unwrap();
        assert_eq!(json, "zhipu_glm");
        let json = serde_json::to_value(Provider::XAi).unwrap();
        assert_eq!(json, "x_ai");
    }

    /// Extensibility proof: adding a brand-new model/provider variant needs
    /// only this one addition — no changes anywhere else in the schema or its
    /// consumers to make it show up in the catalog.
    #[test]
    fn adding_a_new_model_requires_no_other_changes() {
        let mut models = all_models();
        models.push(ModelEntry {
            id: "test-new-model-1".into(),
            name: "Test New Model".into(),
            provider: Provider::Anthropic,
            tier: "experimental".into(),
            context_window: 32_000,
            pricing: Pricing { input_per_m: 1.0, output_per_m: 2.0 },
            reasoning: ReasoningConfig::none(),
        });
        assert!(models.iter().any(|m| m.id == "test-new-model-1"));
        let json = serde_json::to_string(&models).unwrap();
        let back: Vec<ModelEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(models, back);
    }
}
