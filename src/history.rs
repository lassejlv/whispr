//! Local SQLite-backed recording history.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::data_dir;

const SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, Clone)]
pub struct RecordingSummary {
    pub created_at: i64,
    pub duration_ms: i64,
    pub output_text: String,
    pub audio_bytes: usize,
}

pub struct HistoryStore {
    conn: Connection,
}

impl HistoryStore {
    pub fn open() -> Result<Self> {
        let path = history_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }

        let conn = Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS recordings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL,
                sample_rate INTEGER NOT NULL,
                raw_transcript TEXT NOT NULL,
                output_text TEXT NOT NULL,
                audio_wav BLOB NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_recordings_created_at
                ON recordings(created_at DESC);
            ",
        )
        .context("initialize recording history schema")?;

        Ok(Self { conn })
    }

    pub fn save_recording(
        &mut self,
        samples_16k_mono: &[f32],
        raw_transcript: &str,
        output_text: &str,
    ) -> Result<()> {
        let audio_wav = encode_wav_pcm16(samples_16k_mono);
        let duration_ms = (samples_16k_mono.len() as i64 * 1_000) / SAMPLE_RATE as i64;
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock before unix epoch")?
            .as_secs() as i64;

        self.conn
            .execute(
                "
                INSERT INTO recordings (
                    created_at,
                    duration_ms,
                    sample_rate,
                    raw_transcript,
                    output_text,
                    audio_wav
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    created_at,
                    duration_ms,
                    SAMPLE_RATE as i64,
                    raw_transcript,
                    output_text,
                    audio_wav
                ],
            )
            .context("insert recording history")?;

        Ok(())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<RecordingSummary>> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT created_at, duration_ms, output_text, length(audio_wav)
                FROM recordings
                ORDER BY created_at DESC
                LIMIT ?1
                ",
            )
            .context("prepare recording history query")?;

        let rows = stmt
            .query_map([limit as i64], |row| {
                let audio_bytes: i64 = row.get(3)?;
                Ok(RecordingSummary {
                    created_at: row.get(0)?,
                    duration_ms: row.get(1)?,
                    output_text: row.get(2)?,
                    audio_bytes: audio_bytes.max(0) as usize,
                })
            })
            .context("query recording history")?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("read recording history rows")
    }
}

pub fn history_path() -> PathBuf {
    data_dir().join("recordings.sqlite3")
}

fn encode_wav_pcm16(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() * 2;
    let mut out = Vec::with_capacity(44 + data_len);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36_u32 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());

    for sample in samples {
        let sample = sample.clamp(-1.0, 1.0);
        let pcm = (sample * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&pcm.to_le_bytes());
    }

    out
}
