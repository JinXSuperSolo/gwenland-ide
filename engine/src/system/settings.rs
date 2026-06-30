use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const SETTINGS_CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeSettings {
    #[serde(default = "default_theme_mode")]
    pub mode: String,
}

fn default_theme_mode() -> String {
    "system".to_string()
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            mode: default_theme_mode(),
        }
    }
}

/// A user-configured OpenAI-compatible endpoint (Requirement 3.8). The `id`
/// doubles as the keychain account id so each generic endpoint has its own key.
/// `extra_headers` is a map so quirks (OpenRouter `HTTP-Referer`/`X-Title`,
/// custom auth, etc.) live in config rather than bespoke provider code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GenericProviderSetting {
    pub id: String,
    pub display_name: String,
    pub base_url: String,
    pub default_model: String,
    #[serde(default)]
    pub extra_headers: std::collections::BTreeMap<String, String>,
}

/// `Settings.ai`: provider/model preferences, generic provider config, and the
/// training opt-in flag. NEVER contains API keys (those live in the keychain).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiSettings {
    #[serde(default = "default_active_provider")]
    pub active_provider: String,
    #[serde(default)]
    pub active_model: String,
    #[serde(default)]
    pub training_opt_in: bool,
    #[serde(default)]
    pub generic_providers: Vec<GenericProviderSetting>,
}

fn default_active_provider() -> String {
    "anthropic".to_string()
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            active_provider: default_active_provider(),
            active_model: String::new(),
            training_opt_in: false,
            generic_providers: Vec::new(),
        }
    }
}

// All future fields MUST use `#[serde(default)]`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub theme: ThemeSettings,
    #[serde(default)]
    pub ai: AiSettings,
    /// M6 LSP Bridge config (per-language command/args overrides). Defaulted so
    /// M1–M5 settings files without an `[lsp]` table still load (Requirement
    /// 4.2/4.5). Defined in `crate::lsp::config`; never stores secrets.
    #[serde(default)]
    pub lsp: crate::lsp::config::LspSettings,
}

fn default_version() -> u32 {
    SETTINGS_CURRENT_VERSION
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_CURRENT_VERSION,
            theme: ThemeSettings::default(),
            ai: AiSettings::default(),
            lsp: crate::lsp::config::LspSettings::default(),
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("app data directory unavailable: {0}")]
    AppData(#[from] crate::app_data::AppDataError),
    #[error("failed to serialize settings: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("I/O error reading or writing settings: {0}")]
    Io(#[from] std::io::Error),
}

pub fn settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.toml")
}

pub fn load_settings() -> Result<Settings, SettingsError> {
    let app_data_dir = crate::app_data::get_app_data_dir()?;
    load_settings_from(&app_data_dir)
}

fn load_settings_from(app_data_dir: &Path) -> Result<Settings, SettingsError> {
    let path = settings_path(app_data_dir);

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Settings::default()),
        Err(err) => return Err(err.into()),
    };

    let settings: Settings = match toml::from_str(&content) {
        Ok(s) => s,
        Err(_) => return Ok(Settings::default()),
    };

    if settings.theme.mode != "dark"
        && settings.theme.mode != "light"
        && settings.theme.mode != "system"
    {
        return Ok(Settings::default());
    }

    Ok(settings)
}

pub fn save_settings(settings: &Settings) -> Result<(), SettingsError> {
    let app_data_dir = crate::app_data::get_app_data_dir()?;
    save_settings_to(settings, &app_data_dir)
}

fn save_settings_to(settings: &Settings, app_data_dir: &Path) -> Result<(), SettingsError> {
    std::fs::create_dir_all(app_data_dir)?;

    let path = settings_path(app_data_dir);
    let tmp_path = path.with_extension("toml.tmp");

    let content = toml::to_string(settings)?;
    std::fs::write(&tmp_path, content)?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    std::fs::rename(&tmp_path, &path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::{TempDir, tempdir};

    fn isolated_app_data_dir() -> TempDir {
        tempdir().expect("temp app-data dir")
    }

    #[test]
    fn ai_settings_defaults() {
        let ai = AiSettings::default();
        assert_eq!(ai.active_provider, "anthropic");
        assert_eq!(ai.active_model, "");
        assert!(!ai.training_opt_in);
        assert!(ai.generic_providers.is_empty());
    }

    /// Old M1-M3 settings TOML (no `[ai]` table) must still load, filling in AI
    /// defaults (Requirement 3.5 / 20.3).
    #[test]
    fn old_settings_without_ai_loads_with_defaults() {
        let old = "version = 1\n\n[theme]\nmode = \"dark\"\n";
        let settings: Settings = toml::from_str(old).unwrap();
        assert_eq!(settings.theme.mode, "dark");
        assert_eq!(settings.ai, AiSettings::default());
    }

    /// A fully empty file loads as the full default (every field is optional).
    #[test]
    fn empty_settings_loads_full_default() {
        let settings: Settings = toml::from_str("").unwrap();
        assert_eq!(settings, Settings::default());
    }

    /// Old M1–M5 settings TOML (no `[lsp]` table) must still load, filling in
    /// default LSP settings (Requirement 4.2/4.5, task 1.8).
    #[test]
    fn old_settings_without_lsp_loads_with_defaults() {
        let old = "version = 1\n\n[theme]\nmode = \"dark\"\n\n[ai]\nactive_provider = \"openai\"\n";
        let settings: Settings = toml::from_str(old).unwrap();
        assert_eq!(settings.theme.mode, "dark");
        assert_eq!(settings.ai.active_provider, "openai");
        assert_eq!(settings.lsp, crate::lsp::config::LspSettings::default());
    }

    /// LSP settings round-trip through the full Settings struct via TOML.
    #[test]
    fn settings_with_lsp_round_trips() {
        let mut s = Settings::default();
        s.lsp.python.command = "pylsp".into();
        s.lsp.rust.enabled = false;
        let toml_str = toml::to_string(&s).unwrap();
        let back: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn settings_with_ai_round_trips_through_toml() {
        let mut s = Settings::default();
        s.ai.active_provider = "openai".into();
        s.ai.active_model = "gpt-4o".into();
        s.ai.generic_providers.push(GenericProviderSetting {
            id: "generic-groq".into(),
            display_name: "Groq".into(),
            base_url: "https://api.groq.com/openai/v1".into(),
            default_model: "llama-3.1-70b".into(),
            extra_headers: Default::default(),
        });
        let toml_str = toml::to_string(&s).unwrap();
        let back: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn missing_settings_file_returns_default() {
        let app_data_dir = isolated_app_data_dir();
        let loaded = load_settings_from(app_data_dir.path()).unwrap();

        assert_eq!(loaded, Settings::default());
        assert!(!settings_path(app_data_dir.path()).exists());
    }

    #[test]
    fn empty_settings_file_returns_default() {
        let app_data_dir = isolated_app_data_dir();
        std::fs::write(settings_path(app_data_dir.path()), "").unwrap();

        let loaded = load_settings_from(app_data_dir.path()).unwrap();

        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn malformed_settings_file_returns_default() {
        let app_data_dir = isolated_app_data_dir();
        std::fs::write(settings_path(app_data_dir.path()), "not = [valid").unwrap();

        let loaded = load_settings_from(app_data_dir.path()).unwrap();

        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn invalid_theme_mode_returns_default() {
        let app_data_dir = isolated_app_data_dir();
        std::fs::write(
            settings_path(app_data_dir.path()),
            "version = 1\n\n[theme]\nmode = \"neon\"\n",
        )
        .unwrap();

        let loaded = load_settings_from(app_data_dir.path()).unwrap();

        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn save_settings_persists_actual_values_and_overwrites() {
        let app_data_dir = isolated_app_data_dir();
        let mut first = Settings::default();
        first.theme.mode = "dark".to_string();
        first.ai.active_provider = "openai".to_string();
        first.ai.active_model = "gpt-4o".to_string();
        save_settings_to(&first, app_data_dir.path()).unwrap();

        let mut second = first.clone();
        second.theme.mode = "light".to_string();
        second.ai.training_opt_in = true;
        save_settings_to(&second, app_data_dir.path()).unwrap();

        let loaded = load_settings_from(app_data_dir.path()).unwrap();
        let raw = std::fs::read_to_string(settings_path(app_data_dir.path())).unwrap();

        assert_eq!(loaded, second);
        assert!(raw.contains("mode = \"light\""));
        assert!(raw.contains("training_opt_in = true"));
        assert!(!raw.contains("mode = \"dark\""));
    }

    proptest! {
        #[test]
        fn test_settings_roundtrip(mode in prop_oneof![Just("dark".to_string()), Just("light".to_string()), Just("system".to_string())]) {
            let app_data_dir = isolated_app_data_dir();
            let s = Settings { version: 1, theme: ThemeSettings { mode }, ai: AiSettings::default(), lsp: Default::default() };
            save_settings_to(&s, app_data_dir.path()).unwrap();
            let loaded = load_settings_from(app_data_dir.path()).unwrap();
            assert_eq!(s, loaded);
        }
    }
}
