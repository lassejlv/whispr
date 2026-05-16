//! OpenAI cloud STT backend.
//!
//! Wraps `POST https://api.openai.com/v1/audio/transcriptions` with a multipart
//! upload of the captured audio re-encoded as a 16 kHz mono 16-bit WAV. Runs an
//! owned tokio current-thread runtime so the synchronous `transcribe` API stays
//! the same as the Parakeet backend.

use anyhow::{Context, Result, anyhow};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::time::Duration;
use tokio::runtime::Runtime;

const ENDPOINT: &str = "https://api.openai.com/v1/audio/transcriptions";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub struct OpenAiStt {
    api_key: String,
    model: String,
    language: Option<String>,
    client: reqwest::Client,
    rt: Runtime,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl OpenAiStt {
    pub fn new(api_key: String, model: String, language: Option<String>) -> Result<Self> {
        if api_key.trim().is_empty() {
            return Err(anyhow!("OpenAI API key is empty"));
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("build tokio runtime for OpenAI backend")?;
        let client = reqwest::Client::builder()
            .user_agent(concat!("yap/", env!("CARGO_PKG_VERSION")))
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("build reqwest client")?;
        Ok(Self {
            api_key,
            model,
            language,
            client,
            rt,
        })
    }

    pub fn transcribe(&mut self, samples_16k_mono: &[f32]) -> Result<String> {
        let wav = encode_wav_16k_mono(samples_16k_mono);
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let language = self.language.clone();

        let text = self.rt.block_on(async move {
            let part = Part::bytes(wav)
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .context("attach wav mime")?;
            let mut form = Form::new()
                .part("file", part)
                .text("model", model)
                .text("response_format", "json");
            if let Some(lang) = language.filter(|s| !s.trim().is_empty()) {
                form = form.text("language", lang);
            }

            let resp = client
                .post(ENDPOINT)
                .bearer_auth(api_key)
                .multipart(form)
                .send()
                .await
                .context("POST transcriptions")?;

            let status = resp.status();
            let body = resp.text().await.context("read response body")?;
            if !status.is_success() {
                return Err(anyhow!("OpenAI {}: {}", status, truncate(&body, 240)));
            }
            let parsed: TranscriptionResponse =
                serde_json::from_str(&body).with_context(|| {
                    format!(
                        "parse transcription JSON (body starts with: {})",
                        truncate(&body, 120)
                    )
                })?;
            Ok::<_, anyhow::Error>(parsed.text)
        })?;

        Ok(text.trim().to_string())
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

/// Encode f32 PCM samples (range [-1.0, 1.0], 16 kHz mono) as a RIFF/WAVE byte buffer.
fn encode_wav_16k_mono(samples: &[f32]) -> Vec<u8> {
    let sample_rate: u32 = 16_000;
    let bits_per_sample: u16 = 16;
    let channels: u16 = 1;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_len = (samples.len() * 2) as u32;
    let riff_len = 36 + data_len;

    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let i = (clamped * i16::MAX as f32) as i16;
        out.extend_from_slice(&i.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode_wav_16k_mono;

    #[test]
    fn wav_header_is_44_bytes_for_empty_input() {
        let bytes = encode_wav_16k_mono(&[]);
        assert_eq!(bytes.len(), 44);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[36..40], b"data");
    }

    #[test]
    fn wav_encodes_full_scale_samples() {
        let bytes = encode_wav_16k_mono(&[1.0, -1.0]);
        assert_eq!(bytes.len(), 44 + 4);
        let s0 = i16::from_le_bytes([bytes[44], bytes[45]]);
        let s1 = i16::from_le_bytes([bytes[46], bytes[47]]);
        assert_eq!(s0, i16::MAX);
        // -1.0 * i16::MAX rounds to -32767 (one short of i16::MIN).
        assert_eq!(s1, -i16::MAX);
    }
}
