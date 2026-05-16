//! Parakeet TDT offline transducer via sherpa-rs.

use anyhow::{Context, Result, anyhow};
use sherpa_rs::transducer::{TransducerConfig, TransducerRecognizer};
use std::path::{Path, PathBuf};

pub struct ParakeetStt {
    recognizer: TransducerRecognizer,
}

pub struct ModelPaths {
    pub root: PathBuf,
}

impl ModelPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn encoder(&self) -> PathBuf {
        self.root.join("encoder.int8.onnx")
    }
    pub fn decoder(&self) -> PathBuf {
        self.root.join("decoder.int8.onnx")
    }
    pub fn joiner(&self) -> PathBuf {
        self.root.join("joiner.int8.onnx")
    }
    pub fn tokens(&self) -> PathBuf {
        self.root.join("tokens.txt")
    }

    pub fn is_complete(&self) -> bool {
        self.encoder().exists()
            && self.decoder().exists()
            && self.joiner().exists()
            && self.tokens().exists()
            && file_size(&self.encoder()) > 500_000_000
            && file_size(&self.decoder()) > 5_000_000
            && file_size(&self.joiner()) > 1_000_000
    }
}

fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

impl ParakeetStt {
    pub fn load(paths: &ModelPaths) -> Result<Self> {
        if !paths.is_complete() {
            return Err(anyhow!("model files missing in {}", paths.root.display()));
        }
        let config = TransducerConfig {
            encoder: path_str(&paths.encoder())?,
            decoder: path_str(&paths.decoder())?,
            joiner: path_str(&paths.joiner())?,
            tokens: path_str(&paths.tokens())?,
            num_threads: 2,
            sample_rate: 16_000,
            feature_dim: 80,
            decoding_method: "greedy_search".to_string(),
            modeling_unit: "cjkchar".to_string(),
            debug: false,
            model_type: "nemo_transducer".to_string(),
            ..Default::default()
        };
        let recognizer =
            TransducerRecognizer::new(config).map_err(|e| anyhow!("sherpa init failed: {e:?}"))?;
        Ok(Self { recognizer })
    }

    pub fn transcribe(&mut self, samples_16k_mono: &[f32]) -> String {
        let raw = self.recognizer.transcribe(16_000, samples_16k_mono);
        raw.trim().to_string()
    }

    pub fn self_test(&mut self, paths: &ModelPaths) -> Result<Option<String>> {
        let wav = paths.root.join("test_wavs").join("0.wav");
        if !wav.exists() {
            return Ok(None);
        }
        let samples = read_pcm16_wav_mono_16k(&wav)?;
        Ok(Some(self.transcribe(&samples)))
    }
}

fn path_str(p: &Path) -> Result<String> {
    p.to_str()
        .map(|s| s.to_string())
        .with_context(|| format!("non-utf8 path: {}", p.display()))
}

fn read_pcm16_wav_mono_16k(path: &Path) -> Result<Vec<f32>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(anyhow!("not a RIFF/WAVE file: {}", path.display()));
    }

    let mut offset = 12;
    let mut channels = 0_u16;
    let mut sample_rate = 0_u32;
    let mut bits_per_sample = 0_u16;
    let mut data: Option<&[u8]> = None;

    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let len = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        if offset + len > bytes.len() {
            break;
        }
        let chunk = &bytes[offset..offset + len];
        match id {
            b"fmt " if len >= 16 => {
                let audio_format = u16::from_le_bytes(chunk[0..2].try_into().unwrap());
                channels = u16::from_le_bytes(chunk[2..4].try_into().unwrap());
                sample_rate = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
                bits_per_sample = u16::from_le_bytes(chunk[14..16].try_into().unwrap());
                if audio_format != 1 {
                    return Err(anyhow!("unsupported wav format {audio_format}"));
                }
            }
            b"data" => data = Some(chunk),
            _ => {}
        }
        offset += len + (len % 2);
    }

    if channels != 1 || sample_rate != 16_000 || bits_per_sample != 16 {
        return Err(anyhow!(
            "expected mono 16-bit 16 kHz wav, got channels={channels}, sample_rate={sample_rate}, bits={bits_per_sample}"
        ));
    }

    let data = data.ok_or_else(|| anyhow!("wav has no data chunk: {}", path.display()))?;
    Ok(data
        .chunks_exact(2)
        .map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / i16::MAX as f32)
        .collect())
}
