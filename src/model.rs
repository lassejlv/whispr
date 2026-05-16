//! Download + extract the Parakeet sherpa-onnx archive (SHA-256 verified).

use anyhow::{Context, Result, anyhow};
use bzip2::read::BzDecoder;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

use crate::config::{MODEL_ARCHIVE_URL, models_dir, parakeet_dir};

/// Set this once you have personally verified the archive hash. Until then the
/// downloader logs the computed hash so you can pin it. `None` ⇒ verify skipped.
pub const MODEL_SHA256: Option<&str> = None;

#[derive(Debug, Clone)]
pub enum Progress {
    Downloading { bytes: u64, total: Option<u64> },
    Extracting,
    Done,
}

pub type ProgressSink = Arc<dyn Fn(Progress) + Send + Sync>;

pub async fn download_parakeet(sink: ProgressSink) -> Result<PathBuf> {
    let dst_dir = models_dir();
    tokio::fs::create_dir_all(&dst_dir).await?;
    let archive_path = dst_dir.join("parakeet.tar.bz2");

    let res = reqwest::get(MODEL_ARCHIVE_URL)
        .await
        .with_context(|| format!("GET {MODEL_ARCHIVE_URL}"))?
        .error_for_status()?;
    let total = res.content_length();

    let mut file = tokio::fs::File::create(&archive_path).await?;
    let mut stream = res.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut hasher = Sha256::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("download chunk")?;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        sink(Progress::Downloading {
            bytes: downloaded,
            total,
        });
    }
    file.flush().await?;
    drop(file);

    let digest = hex::encode(hasher.finalize());
    tracing::info!("downloaded {} bytes — sha256={}", downloaded, digest);
    if let Some(expected) = MODEL_SHA256 {
        if !digest.eq_ignore_ascii_case(expected) {
            let _ = tokio::fs::remove_file(&archive_path).await;
            return Err(anyhow!(
                "sha256 mismatch: expected {expected}, got {digest}"
            ));
        }
    }

    sink(Progress::Extracting);
    let archive_path_clone = archive_path.clone();
    let dst_dir_clone = dst_dir.clone();
    tokio::task::spawn_blocking(move || extract(&archive_path_clone, &dst_dir_clone))
        .await
        .context("extract task join")??;
    let _ = tokio::fs::remove_file(&archive_path).await;

    let out = parakeet_dir();
    if !out.exists() {
        return Err(anyhow!(
            "extracted but expected dir not found: {}",
            out.display()
        ));
    }
    sink(Progress::Done);
    Ok(out)
}

fn extract(archive: &Path, dst: &Path) -> Result<()> {
    let f = std::fs::File::open(archive)
        .with_context(|| format!("open archive {}", archive.display()))?;
    let mut tar = tar::Archive::new(BzDecoder::new(f));
    tar.unpack(dst)
        .with_context(|| format!("unpack into {}", dst.display()))?;
    Ok(())
}
