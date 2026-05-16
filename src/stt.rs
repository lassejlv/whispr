//! STT backend dispatcher.
//!
//! Both backends accept the same input (f32 mono PCM at 16 kHz) and return a
//! trimmed transcript. The Parakeet backend runs locally via sherpa-onnx; the
//! OpenAI backend uploads a WAV to /v1/audio/transcriptions.

mod openai;
mod parakeet;

pub use openai::OpenAiStt;
pub use parakeet::{ModelPaths, ParakeetStt};

use anyhow::Result;

pub enum Stt {
    Parakeet(ParakeetStt),
    OpenAi(OpenAiStt),
}

impl Stt {
    pub fn transcribe(&mut self, samples_16k_mono: &[f32]) -> Result<String> {
        match self {
            Stt::Parakeet(p) => Ok(p.transcribe(samples_16k_mono)),
            Stt::OpenAi(o) => o.transcribe(samples_16k_mono),
        }
    }

    /// Self-test only applies to the Parakeet backend. Returns `Ok(None)` for
    /// OpenAI since loading just stores credentials, with no model to exercise.
    pub fn self_test(&mut self, paths: &ModelPaths) -> Result<Option<String>> {
        match self {
            Stt::Parakeet(p) => p.self_test(paths),
            Stt::OpenAi(_) => Ok(None),
        }
    }

}
