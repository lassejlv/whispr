pub mod openai;
pub mod parakeet;

use anyhow::Result;
use async_trait::async_trait;

use crate::audio::wav_bytes;
use crate::config::{Config, SttBackend};

#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(&self, samples: Vec<i16>) -> Result<String>;
}

pub fn build(cfg: &Config) -> Box<dyn Transcriber> {
    match cfg.stt_backend {
        SttBackend::OpenAi => Box::new(OpenAiTranscriber {
            client: openai::SttClient::new(cfg.openai_api_key.clone(), cfg.stt_model.clone()),
        }),
        SttBackend::Parakeet => Box::new(ParakeetTranscriber),
    }
}

struct OpenAiTranscriber {
    client: openai::SttClient,
}

#[async_trait]
impl Transcriber for OpenAiTranscriber {
    async fn transcribe(&self, samples: Vec<i16>) -> Result<String> {
        let wav = wav_bytes(&samples)?;
        self.client.transcribe(wav, None).await
    }
}

struct ParakeetTranscriber;

#[async_trait]
impl Transcriber for ParakeetTranscriber {
    async fn transcribe(&self, samples: Vec<i16>) -> Result<String> {
        tokio::task::spawn_blocking(move || parakeet::transcribe(&samples)).await?
    }
}
