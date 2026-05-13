use anyhow::{anyhow, Result};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use scriptrs::LongFormTranscriptionPipeline;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelStatus {
    NotLoaded,
    Downloading,
    Ready,
    Failed(String),
}

impl ModelStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, ModelStatus::Ready)
    }

    pub fn is_busy(&self) -> bool {
        matches!(self, ModelStatus::Downloading)
    }
}

static STATUS: OnceCell<Arc<RwLock<ModelStatus>>> = OnceCell::new();

fn status_cell() -> &'static Arc<RwLock<ModelStatus>> {
    STATUS.get_or_init(|| Arc::new(RwLock::new(ModelStatus::NotLoaded)))
}

pub fn status() -> ModelStatus {
    status_cell().read().clone()
}

fn set_status(s: ModelStatus) {
    *status_cell().write() = s;
}

enum Job {
    Preload(Sender<Result<()>>),
    Transcribe {
        samples: Vec<f32>,
        response: Sender<Result<String>>,
    },
}

struct Worker {
    tx: Sender<Job>,
}

static WORKER: OnceCell<Worker> = OnceCell::new();

fn worker() -> &'static Worker {
    WORKER.get_or_init(|| {
        let _ = status_cell();
        let (tx, rx) = unbounded::<Job>();
        std::thread::Builder::new()
            .name("parakeet-worker".into())
            .spawn(move || run_worker(rx))
            .expect("spawn parakeet worker");
        Worker { tx }
    })
}

fn ensure_pipeline(
    pipeline: &mut Option<LongFormTranscriptionPipeline>,
) -> std::result::Result<(), String> {
    if pipeline.is_some() {
        return Ok(());
    }
    if !status().is_ready() {
        set_status(ModelStatus::Downloading);
    }
    tracing::info!("loading Parakeet model");
    match LongFormTranscriptionPipeline::from_pretrained() {
        Ok(p) => {
            *pipeline = Some(p);
            set_status(ModelStatus::Ready);
            tracing::info!("Parakeet model ready");
            Ok(())
        }
        Err(e) => {
            let msg = format!("{e}");
            set_status(ModelStatus::Failed(msg.clone()));
            Err(msg)
        }
    }
}

fn run_worker(rx: Receiver<Job>) {
    let mut pipeline: Option<LongFormTranscriptionPipeline> = None;

    while let Ok(job) = rx.recv() {
        match job {
            Job::Preload(resp) => {
                let result = ensure_pipeline(&mut pipeline)
                    .map_err(|e| anyhow!("Parakeet init failed: {e}"));
                let _ = resp.send(result);
            }
            Job::Transcribe { samples, response } => {
                if let Err(e) = ensure_pipeline(&mut pipeline) {
                    let _ = response.send(Err(anyhow!("Parakeet init failed: {e}")));
                    continue;
                }
                let pipe = pipeline.as_mut().unwrap();
                let result = match pipe.run(&samples) {
                    Ok(r) => Ok(r.text.trim().to_string()),
                    Err(e) => Err(anyhow!("Parakeet inference: {e}")),
                };
                let _ = response.send(result);
            }
        }
    }
}

pub fn preload() -> Result<()> {
    if status().is_ready() {
        return Ok(());
    }
    let (resp_tx, resp_rx) = bounded(1);
    worker()
        .tx
        .send(Job::Preload(resp_tx))
        .map_err(|_| anyhow!("Parakeet worker dropped"))?;
    resp_rx
        .recv()
        .map_err(|_| anyhow!("Parakeet worker exited"))?
}

pub fn transcribe(samples: &[i16]) -> Result<String> {
    let audio: Vec<f32> = samples
        .iter()
        .map(|s| *s as f32 / i16::MAX as f32)
        .collect();

    let (resp_tx, resp_rx) = bounded(1);
    worker()
        .tx
        .send(Job::Transcribe {
            samples: audio,
            response: resp_tx,
        })
        .map_err(|_| anyhow!("Parakeet worker dropped"))?;
    resp_rx
        .recv()
        .map_err(|_| anyhow!("Parakeet worker exited"))?
}
