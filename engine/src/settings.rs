//! User preferences shared by the window, the CLI and the MCP server. Stored as JSON in
//! the application data directory (`settings.json`, readable only by the user because it
//! may hold API keys). Every field has a default so an older or partial file still loads,
//! and every write validates the whole document first so the file is never half-broken.

use crate::{host::scan::data_dir, plugin::Format, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub const VERSION: u32 = 1;
const MASKED: &str = "••••";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    pub general: General,
    pub audio: Audio,
    pub interface: Interface,
    pub agent: Agent,
    pub plugins: Plugins,
    pub control: Control,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct General {
    pub check_updates_on_start: bool,
    pub install_updates_automatically: bool,
    /// Seconds between recovery snapshots while an edited session is idle (10-600).
    pub recovery_interval_seconds: u32,
    pub confirm_before_quit: bool,
    pub reopen_last_session: bool,
    pub last_session: Option<String>,
    pub recent_sessions: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Audio {
    pub output_device: Option<String>,
    pub input_device: Option<String>,
    pub midi_input: Option<String>,
    pub connect_midi_on_start: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Interface {
    /// Interface zoom, 0.75-1.75.
    pub scale: f32,
    pub agent_panel_open_on_start: bool,
    pub show_tooltips: bool,
    pub follow_playhead: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// The installed Codex CLI with its own sign-in.
    Codex,
    /// The installed Claude Code CLI with its own sign-in.
    Claude,
    /// The Anthropic Messages API with an API key.
    Anthropic,
    /// The OpenAI API with an API key.
    #[serde(rename = "openai")]
    OpenAi,
    /// Any OpenAI-compatible endpoint (local models, other vendors).
    Compatible,
}
impl Provider {
    pub const ALL: [Provider; 5] = [
        Provider::Codex,
        Provider::Claude,
        Provider::Anthropic,
        Provider::OpenAi,
        Provider::Compatible,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Provider::Codex => "Codex CLI (OpenAI sign-in)",
            Provider::Claude => "Claude Code CLI (Anthropic sign-in)",
            Provider::Anthropic => "Anthropic API key",
            Provider::OpenAi => "OpenAI API key",
            Provider::Compatible => "OpenAI-compatible endpoint",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Provider::Codex => "codex",
            Provider::Claude => "claude",
            Provider::Anthropic => "anthropic",
            Provider::OpenAi => "openai",
            Provider::Compatible => "compatible",
        }
    }
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.key() == key)
    }
    /// The model used when the settings leave it blank.
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::Codex => "",
            Provider::Claude => "",
            Provider::Anthropic => "claude-sonnet-5",
            Provider::OpenAi => "gpt-5",
            Provider::Compatible => "",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Agent {
    pub provider: Provider,
    /// Model name; blank uses the provider's default.
    pub model: String,
    pub anthropic_api_key: String,
    pub openai_api_key: String,
    pub compatible_base_url: String,
    pub compatible_api_key: String,
    pub codex_executable: String,
    pub claude_executable: String,
    /// Largest reply the model may produce per turn.
    pub max_output_tokens: u32,
    /// Tool calls allowed in one task before the agent must answer.
    pub max_tool_rounds: u32,
    /// Extra standing instructions appended to the system prompt.
    pub instructions: String,
    pub permissions: Permissions,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Permissions {
    /// session.save/open/import/export/bounce and plugin.scan.
    pub file_operations: bool,
    /// transport.play/record/stop/locate.
    pub transport: bool,
    /// session.new and session.open replace the document.
    pub replace_session: bool,
    /// settings.set / settings.reset.
    pub settings: bool,
    /// app.quit and app.installUpdate.
    pub app_control: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Plugins {
    pub scan_on_start: bool,
    pub extra_clap_paths: Vec<String>,
    pub extra_vst3_paths: Vec<String>,
    pub extra_native_paths: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Control {
    /// Serve the CLI/MCP bridge when the window starts.
    pub enable_bridge: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: VERSION,
            general: General::default(),
            audio: Audio::default(),
            interface: Interface::default(),
            agent: Agent::default(),
            plugins: Plugins::default(),
            control: Control::default(),
        }
    }
}
impl Default for General {
    fn default() -> Self {
        Self {
            check_updates_on_start: true,
            install_updates_automatically: false,
            recovery_interval_seconds: 30,
            confirm_before_quit: true,
            reopen_last_session: false,
            last_session: None,
            recent_sessions: vec![],
        }
    }
}
impl Default for Audio {
    fn default() -> Self {
        Self {
            output_device: None,
            input_device: None,
            midi_input: None,
            connect_midi_on_start: true,
        }
    }
}
impl Default for Interface {
    fn default() -> Self {
        Self {
            scale: 1.0,
            agent_panel_open_on_start: false,
            show_tooltips: true,
            follow_playhead: true,
        }
    }
}
impl Default for Agent {
    fn default() -> Self {
        Self {
            provider: Provider::Codex,
            model: String::new(),
            anthropic_api_key: String::new(),
            openai_api_key: String::new(),
            compatible_base_url: String::new(),
            compatible_api_key: String::new(),
            codex_executable: String::new(),
            claude_executable: String::new(),
            max_output_tokens: 4096,
            max_tool_rounds: 48,
            instructions: String::new(),
            permissions: Permissions::default(),
        }
    }
}
impl Default for Permissions {
    fn default() -> Self {
        Self {
            file_operations: true,
            transport: true,
            replace_session: false,
            settings: false,
            app_control: false,
        }
    }
}
impl Default for Control {
    fn default() -> Self {
        Self {
            enable_bridge: true,
        }
    }
}

/// Dotted paths of fields that hold secrets.
pub const SECRET_PATHS: [&str; 3] = [
    "agent.anthropicApiKey",
    "agent.openaiApiKey",
    "agent.compatibleApiKey",
];

impl Settings {
    /// `$ONDERA_SETTINGS`, else `settings.json` in the data directory.
    pub fn path() -> PathBuf {
        if let Some(p) = std::env::var_os("ONDERA_SETTINGS").filter(|p| !p.is_empty()) {
            return PathBuf::from(p);
        }
        data_dir().join("settings.json")
    }
    /// The stored settings, or defaults when the file is absent or unreadable.
    pub fn load() -> Self {
        Self::read(&Self::path()).unwrap_or_default()
    }
    /// The stored settings, reporting an unreadable file instead of hiding it.
    pub fn read(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                if text.len() > 4 * 1024 * 1024 {
                    return Err("Settings file exceeds 4 MiB".into());
                }
                let settings: Settings = serde_json::from_str(&text)
                    .map_err(|e| format!("Invalid settings file {}: {e}", path.display()))?;
                settings.validate()?;
                Ok(settings)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("Cannot read {}: {e}", path.display())),
        }
    }
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path())
    }
    pub fn save_to(&self, path: &Path) -> Result<()> {
        self.validate()?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut stored = self.clone();
        stored.version = VERSION;
        let text = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
        crate::document::atomic_write(path, |f| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                f.set_permissions(std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| e.to_string())?;
            }
            f.write_all(text.as_bytes()).map_err(|e| e.to_string())
        })
    }
    pub fn validate(&self) -> Result<()> {
        if !(10..=600).contains(&self.general.recovery_interval_seconds) {
            return Err("Recovery interval must be 10-600 seconds".into());
        }
        if !self.interface.scale.is_finite() || !(0.75..=1.75).contains(&self.interface.scale) {
            return Err("Interface scale must be between 0.75 and 1.75".into());
        }
        if !(256..=128_000).contains(&self.agent.max_output_tokens) {
            return Err("Agent max output tokens must be 256-128000".into());
        }
        if !(1..=500).contains(&self.agent.max_tool_rounds) {
            return Err("Agent max tool rounds must be 1-500".into());
        }
        if self.agent.instructions.len() > 20_000 {
            return Err("Agent instructions exceed 20,000 characters".into());
        }
        let url = &self.agent.compatible_base_url;
        if !(url.is_empty() || url.starts_with("http://") || url.starts_with("https://")) {
            return Err("The compatible endpoint must be an http(s) URL".into());
        }
        if self.general.recent_sessions.len() > 20 {
            return Err("At most 20 recent sessions are kept".into());
        }
        for key in [
            &self.agent.anthropic_api_key,
            &self.agent.openai_api_key,
            &self.agent.compatible_api_key,
        ] {
            if key.len() > 4096 || key.chars().any(char::is_control) {
                return Err("API keys must be printable and shorter than 4096 characters".into());
            }
        }
        for paths in [
            &self.plugins.extra_clap_paths,
            &self.plugins.extra_vst3_paths,
            &self.plugins.extra_native_paths,
        ] {
            if paths.len() > 64 || paths.iter().any(|p| p.is_empty() || p.len() > 4096) {
                return Err("Plugin search paths must be 1-64 non-empty entries".into());
            }
        }
        Ok(())
    }
    /// Extra scan directories configured for one plugin format.
    pub fn extra_plugin_paths(&self, format: Format) -> Vec<PathBuf> {
        match format {
            Format::Clap => &self.plugins.extra_clap_paths,
            Format::Vst3 => &self.plugins.extra_vst3_paths,
            Format::Native => &self.plugins.extra_native_paths,
            _ => return vec![],
        }
        .iter()
        .map(PathBuf::from)
        .collect()
    }
    /// The API key for a provider: settings first, then the conventional environment variable.
    pub fn api_key(&self, provider: Provider) -> Option<String> {
        let (stored, env) = match provider {
            Provider::Anthropic => (&self.agent.anthropic_api_key, "ANTHROPIC_API_KEY"),
            Provider::OpenAi => (&self.agent.openai_api_key, "OPENAI_API_KEY"),
            Provider::Compatible => (&self.agent.compatible_api_key, "OPENAI_API_KEY"),
            _ => return None,
        };
        let stored = stored.trim();
        if !stored.is_empty() {
            return Some(stored.to_string());
        }
        std::env::var(env)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }
    /// The model for the configured provider.
    pub fn model(&self) -> String {
        let model = self.agent.model.trim();
        if model.is_empty() {
            self.agent.provider.default_model().to_string()
        } else {
            model.to_string()
        }
    }
    /// The whole document with secrets masked, for display and for the registry.
    pub fn redacted(&self) -> Value {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        for path in SECRET_PATHS {
            if let Some(slot) = lookup_mut(&mut value, path) {
                if let Some(text) = slot.as_str() {
                    *slot = json!(mask(text));
                }
            }
        }
        value
    }
    /// Read a dotted path (`agent.model`) or the whole document when `path` is `None`.
    pub fn get(&self, path: Option<&str>) -> Result<Value> {
        let value = self.redacted();
        match path.map(str::trim).filter(|p| !p.is_empty()) {
            None => Ok(value),
            Some(path) => lookup(&value, path)
                .cloned()
                .ok_or_else(|| format!("Unknown setting `{path}`. Use settings.get to list them.")),
        }
    }
    /// Change one dotted path. Secrets set to their masked display value are left alone.
    pub fn set(&mut self, path: &str, value: Value) -> Result<()> {
        let path = path.trim();
        if path.is_empty() || path == "version" {
            return Err("Choose a setting path such as agent.model".into());
        }
        if SECRET_PATHS.contains(&path) && value.as_str().is_some_and(|s| s.starts_with(MASKED)) {
            return Ok(());
        }
        let mut document = serde_json::to_value(&*self).map_err(|e| e.to_string())?;
        let slot = lookup_mut(&mut document, path)
            .ok_or_else(|| format!("Unknown setting `{path}`. Use settings.get to list them."))?;
        if slot.is_object() {
            return Err(format!("`{path}` is a section; set one of its fields"));
        }
        *slot = coerce(slot, value)?;
        let mut next: Settings = serde_json::from_value(document)
            .map_err(|e| format!("Invalid value for `{path}`: {e}"))?;
        if next.agent.provider != self.agent.provider {
            next.agent.model.clear();
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    /// Reset one path (or everything) to its default.
    pub fn reset(&mut self, path: Option<&str>) -> Result<()> {
        let defaults = Settings::default();
        match path.map(str::trim).filter(|p| !p.is_empty()) {
            None => {
                *self = defaults;
                Ok(())
            }
            Some(path) => {
                let source = serde_json::to_value(&defaults).map_err(|e| e.to_string())?;
                let value = lookup(&source, path)
                    .cloned()
                    .ok_or_else(|| format!("Unknown setting `{path}`"))?;
                if value.is_object() {
                    let mut document = serde_json::to_value(&*self).map_err(|e| e.to_string())?;
                    *lookup_mut(&mut document, path).ok_or("Unknown setting")? = value;
                    *self = serde_json::from_value(document).map_err(|e| e.to_string())?;
                    Ok(())
                } else {
                    self.set(path, value)
                }
            }
        }
    }
}

fn mask(secret: &str) -> String {
    if secret.is_empty() {
        String::new()
    } else if secret.chars().count() <= 6 {
        MASKED.to_string()
    } else {
        let start = secret
            .char_indices()
            .rev()
            .nth(3)
            .map_or(0, |(index, _)| index);
        format!("{MASKED}{}", &secret[start..])
    }
}
fn lookup<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |v, key| v.get(key))
}
fn lookup_mut<'a>(value: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    path.split('.').try_fold(value, |v, key| v.get_mut(key))
}
/// Accept strings for numbers and booleans so `settings.set` works from a shell.
fn coerce(current: &Value, value: Value) -> Result<Value> {
    Ok(match (current, &value) {
        (Value::Bool(_), Value::String(s)) => match s.as_str() {
            "true" | "on" | "yes" | "1" => json!(true),
            "false" | "off" | "no" | "0" => json!(false),
            _ => return Err("Expected true or false".into()),
        },
        (Value::Number(_), Value::String(s)) => {
            let n: f64 = s.parse().map_err(|_| "Expected a number".to_string())?;
            if n.fract() == 0.0 && current.is_u64() {
                json!(n as u64)
            } else {
                json!(n)
            }
        }
        (Value::Null, Value::String(s)) if s.is_empty() => Value::Null,
        (Value::Array(_), Value::String(s)) => {
            json!(s
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>())
        }
        _ => value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn switching_provider_resets_an_incompatible_model_but_keeps_credentials() {
        let mut settings = Settings::default();
        settings.agent.model = "a-codex-model".into();
        settings.agent.openai_api_key = "keep-this-key".into();
        settings.set("agent.provider", json!("codex")).unwrap();
        assert_eq!(settings.agent.model, "a-codex-model");
        settings.set("agent.provider", json!("openai")).unwrap();
        assert!(settings.agent.model.is_empty());
        assert_eq!(settings.agent.openai_api_key, "keep-this-key");
    }

    #[test]
    fn masked_keys_are_unicode_safe_and_do_not_expose_short_keys() {
        for key in [
            "",
            "a",
            "abcdef",
            "é😀短",
            "aé😀longsecret",
            "abcde🦀é文ß",
            "1234567",
        ] {
            let mut settings = Settings::default();
            settings.set("agent.openaiApiKey", json!(key)).unwrap();
            let shown = settings.get(Some("agent.openaiApiKey")).unwrap();
            if key.is_empty() {
                assert_eq!(shown, "");
            } else if key.chars().count() <= 6 {
                assert_eq!(shown, MASKED);
            } else {
                let masked = shown.as_str().unwrap();
                assert!(masked.starts_with(MASKED));
                assert_eq!(masked.chars().count(), 8);
                assert!(!masked.contains(key));
                assert!(key.ends_with(masked.strip_prefix(MASKED).unwrap()));
            }
            settings.set("agent.openaiApiKey", shown).unwrap();
            assert_eq!(settings.agent.openai_api_key, key);
        }
    }
    #[test]
    fn settings_round_trip_redact_secrets_and_validate_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.json");
        let mut settings = Settings::default();
        settings.set("agent.provider", json!("anthropic")).unwrap();
        settings
            .set("agent.anthropicApiKey", json!("sk-ant-secret-1234"))
            .unwrap();
        settings.set("interface.scale", json!("1.25")).unwrap();
        settings
            .set("general.confirmBeforeQuit", json!("off"))
            .unwrap();
        settings
            .set("plugins.extraNativePaths", json!("/a, /b"))
            .unwrap();
        settings.save_to(&path).unwrap();
        let loaded = Settings::read(&path).unwrap();
        assert_eq!(loaded, settings);
        assert_eq!(loaded.agent.provider, Provider::Anthropic);
        assert_eq!(loaded.model(), "claude-sonnet-5");
        assert_eq!(
            loaded.api_key(Provider::Anthropic).unwrap(),
            "sk-ant-secret-1234"
        );
        let shown = loaded.get(Some("agent.anthropicApiKey")).unwrap();
        assert_eq!(shown, json!("••••1234"));
        assert!(!loaded.redacted().to_string().contains("secret"));
        assert_eq!(loaded.plugins.extra_native_paths, vec!["/a", "/b"]);
        let mut again = loaded.clone();
        again
            .set("agent.anthropicApiKey", json!("••••1234"))
            .unwrap();
        assert_eq!(again.agent.anthropic_api_key, "sk-ant-secret-1234");
        assert!(again.set("agent.nonsense", json!(1)).is_err());
        assert!(again.set("interface.scale", json!(9)).is_err());
        assert!(again.set("agent", json!({})).is_err());
        assert_eq!(again.interface.scale, 1.25);
        again.reset(Some("interface")).unwrap();
        assert_eq!(again.interface.scale, 1.0);
        again.reset(None).unwrap();
        assert_eq!(again, Settings::default());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert!(Settings::read(&dir.path().join("absent.json")).unwrap() == Settings::default());
        std::fs::write(&path, "{broken").unwrap();
        assert!(Settings::read(&path).is_err());
    }
}
