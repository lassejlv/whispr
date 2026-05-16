//! Microphone capture → mono f32 @ 16 kHz, accumulated until stop.

use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream, StreamConfig};
use parking_lot::Mutex;
use std::sync::Arc;

const TARGET_SR: u32 = 16_000;

pub struct Recorder {
    stream: Option<Stream>,
    samples: Arc<Mutex<Vec<f32>>>,
    src_rate: u32,
}

impl Recorder {
    pub fn start() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device"))?;
        let supported = device
            .default_input_config()
            .context("query default input config")?;
        let src_rate = supported.sample_rate().0;
        let src_channels = supported.channels();
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();

        let samples = Arc::new(Mutex::new(Vec::<f32>::with_capacity(
            (TARGET_SR * 30) as usize,
        )));
        let buf = samples.clone();
        let err_fn = |e| tracing::error!("audio stream error: {e}");

        let stream = match sample_format {
            SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _| append(data, &buf, src_channels),
                err_fn,
                None,
            )?,
            SampleFormat::I16 => device.build_input_stream(
                &config,
                move |data: &[i16], _| {
                    let f: Vec<f32> = data.iter().map(|s| s.to_sample::<f32>()).collect();
                    append(&f, &buf, src_channels)
                },
                err_fn,
                None,
            )?,
            SampleFormat::U16 => device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    let f: Vec<f32> = data.iter().map(|s| s.to_sample::<f32>()).collect();
                    append(&f, &buf, src_channels)
                },
                err_fn,
                None,
            )?,
            fmt => return Err(anyhow!("unsupported sample format: {fmt:?}")),
        };
        stream.play().context("start input stream")?;

        Ok(Self {
            stream: Some(stream),
            samples,
            src_rate,
        })
    }

    pub fn stop(mut self) -> Vec<f32> {
        if let Some(s) = self.stream.take() {
            drop(s);
        }
        let mono = std::mem::take(&mut *self.samples.lock());
        resample_linear(&mono, self.src_rate, TARGET_SR)
    }

    pub fn current_level(&self) -> f32 {
        let s = self.samples.lock();
        let tail = s.len().saturating_sub(1024);
        let window = &s[tail..];
        if window.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = window.iter().map(|x| x * x).sum();
        (sum_sq / window.len() as f32).sqrt().min(1.0)
    }
}

fn append(data: &[f32], buf: &Arc<Mutex<Vec<f32>>>, channels: u16) {
    if channels <= 1 {
        buf.lock().extend_from_slice(data);
        return;
    }
    let ch = channels as usize;
    let mut out = buf.lock();
    out.reserve(data.len() / ch);
    for frame in data.chunks_exact(ch) {
        let avg = frame.iter().sum::<f32>() / ch as f32;
        out.push(avg);
    }
}

fn resample_linear(input: &[f32], src: u32, dst: u32) -> Vec<f32> {
    if src == dst || input.is_empty() {
        return input.to_vec();
    }
    let ratio = src as f64 / dst as f64;
    let out_len = ((input.len() as f64) / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(input.len() - 1);
        let frac = (pos - i0 as f64) as f32;
        out.push(input[i0] * (1.0 - frac) + input[i1] * frac);
    }
    out
}
