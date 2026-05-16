//! Filesystem layout + persisted settings.

use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

pub const MODEL_DIR_NAME: &str = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8";
pub const MODEL_ARCHIVE_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2";

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join("whispr")
}

pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}

pub fn parakeet_dir() -> PathBuf {
    models_dir().join(MODEL_DIR_NAME)
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// If true, append a trailing space after the inserted transcript.
    #[serde(default = "default_true")]
    pub trailing_space: bool,
    /// Min audio length (samples @ 16k) to bother transcribing.
    #[serde(default = "default_min_samples")]
    pub min_samples: usize,
    /// Push-to-talk modifier key.
    #[serde(default)]
    pub hotkey: Hotkey,
}

pub type SharedSettings = Arc<RwLock<Settings>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Hotkey {
    #[default]
    Fn,
    Option,
    Control,
    Command,
}

impl Hotkey {
    pub const ALL: [Hotkey; 4] = [Hotkey::Fn, Hotkey::Option, Hotkey::Control, Hotkey::Command];

    pub fn label(self) -> &'static str {
        match self {
            Hotkey::Fn => "fn",
            Hotkey::Option => "option",
            Hotkey::Control => "control",
            Hotkey::Command => "command",
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_min_samples() -> usize {
    16_000 / 4 // 250 ms
}

impl Settings {
    pub fn load() -> Self {
        match std::fs::read_to_string(settings_path()) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self {
                trailing_space: true,
                min_samples: 16_000 / 4,
                hotkey: Hotkey::Fn,
            },
        }
    }

    pub fn shared(self) -> SharedSettings {
        Arc::new(RwLock::new(self))
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path();
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).ok();
        }
        let s = serde_json::to_string_pretty(self).context("serialize settings")?;
        std::fs::write(&path, s).with_context(|| format!("write {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::{Hotkey, Settings};

    #[test]
    fn missing_hotkey_defaults_to_fn() {
        let settings: Settings =
            serde_json::from_str(r#"{"trailing_space":true,"min_samples":4000}"#).unwrap();
        assert_eq!(settings.hotkey, Hotkey::Fn);
    }

    #[test]
    fn custom_hotkey_round_trips() {
        let settings = Settings {
            trailing_space: true,
            min_samples: 4000,
            hotkey: Hotkey::Option,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.hotkey, Hotkey::Option);
    }
}
