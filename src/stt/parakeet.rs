use anyhow::{anyhow, Result};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use once_cell::sync::OnceCell;
use scriptrs::LongFormTranscriptionPipeline;

struct Job {
    samples: Vec<f32>,
    response: Sender<Result<String>>,
}

struct Worker {
    tx: Sender<Job>,
}

static WORKER: OnceCell<Worker> = OnceCell::new();

fn worker() -> &'static Worker {
    WORKER.get_or_init(|| {
        let (tx, rx) = unbounded::<Job>();
        std::thread::Builder::new()
            .name("parakeet-worker".into())
            .spawn(move || run_worker(rx))
            .expect("spawn parakeet worker");
        Worker { tx }
    })
}

fn run_worker(rx: Receiver<Job>) {
    let mut pipeline: Option<LongFormTranscriptionPipeline> = None;

    while let Ok(job) = rx.recv() {
        if pipeline.is_none() {
            tracing::info!("loading Parakeet model (first run downloads ~600 MB from Hugging Face)");
            match LongFormTranscriptionPipeline::from_pretrained() {
                Ok(p) => {
                    tracing::info!("Parakeet model ready");
                    pipeline = Some(p);
                }
                Err(e) => {
                    let _ = job
                        .response
                        .send(Err(anyhow!("Parakeet init failed: {e}")));
                    continue;
                }
            }
        }

        let pipe = pipeline.as_mut().unwrap();
        let result = match pipe.run(&job.samples) {
            Ok(r) => Ok(r.text.trim().to_string()),
            Err(e) => Err(anyhow!("Parakeet inference: {e}")),
        };
        let _ = job.response.send(result);
    }
}

pub fn transcribe(samples: &[i16]) -> Result<String> {
    let audio: Vec<f32> = samples
        .iter()
        .map(|s| *s as f32 / i16::MAX as f32)
        .collect();

    let (resp_tx, resp_rx) = bounded(1);
    worker()
        .tx
        .send(Job {
            samples: audio,
            response: resp_tx,
        })
        .map_err(|_| anyhow!("Parakeet worker dropped"))?;

    resp_rx.recv().map_err(|_| anyhow!("Parakeet worker exited"))?
}
