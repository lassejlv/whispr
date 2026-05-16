//! Filesystem layout + persisted settings.

use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

pub const MODEL_DIR_NAME: &str = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8";
pub const MODEL_ARCHIVE_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2";

pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| std::env::temp_dir());
    let new_dir = base.join("yap");
    let legacy = base.join("whispr");
    if !new_dir.exists() && legacy.exists() {
        let _ = std::fs::rename(&legacy, &new_dir);
    }
    new_dir
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
    /// Which STT engine to use.
    #[serde(default)]
    pub backend: BackendKind,
    /// OpenAI transcription model id.
    #[serde(default = "default_openai_model")]
    pub openai_model: String,
    /// Optional ISO-639-1 language hint passed to OpenAI. None = auto-detect.
    #[serde(default)]
    pub openai_language: Option<String>,
}

pub type SharedSettings = Arc<RwLock<Settings>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    #[default]
    OpenAi,
    Parakeet,
}

impl BackendKind {
    pub const ALL: [BackendKind; 2] = [BackendKind::OpenAi, BackendKind::Parakeet];

    pub fn label(self) -> &'static str {
        match self {
            BackendKind::OpenAi => "OpenAI (cloud)",
            BackendKind::Parakeet => "Parakeet (local)",
        }
    }
}

pub const OPENAI_MODELS: &[&str] = &[
    "gpt-4o-transcribe",
    "gpt-4o-mini-transcribe",
    "whisper-1",
];

fn default_openai_model() -> String {
    "gpt-4o-transcribe".to_string()
}

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
            Err(_) => Self::fresh_defaults(),
        }
    }

    fn fresh_defaults() -> Self {
        Self {
            trailing_space: true,
            min_samples: 16_000 / 4,
            hotkey: Hotkey::Fn,
            backend: BackendKind::OpenAi,
            openai_model: default_openai_model(),
            openai_language: None,
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
            backend: super::BackendKind::OpenAi,
            openai_model: super::default_openai_model(),
            openai_language: None,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.hotkey, Hotkey::Option);
    }

    #[test]
    fn defaults_to_openai_backend() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(matches!(s.backend, super::BackendKind::OpenAi));
        assert_eq!(s.openai_model, "gpt-4o-transcribe");
        assert!(s.openai_language.is_none());
    }

    #[test]
    fn legacy_settings_without_backend_default_to_openai() {
        let s: Settings = serde_json::from_str(
            r#"{"trailing_space":true,"min_samples":4000,"hotkey":"fn"}"#,
        )
        .unwrap();
        assert!(matches!(s.backend, super::BackendKind::OpenAi));
    }
}
