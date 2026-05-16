//! Background worker: owns recorder + STT, reacts to hotkey/UI events.

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender, select, tick};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::audio::Recorder;
use crate::config::{SharedSettings, parakeet_dir};
use crate::history::HistoryStore;
use crate::hotkey::HotkeyEvent;
use crate::model::{Progress, ProgressSink, download_parakeet};
use crate::paste::paste_text;
use crate::state::{CoreCmd, CoreEvent, Phase};
use crate::stt::{ModelPaths, Stt};
use crate::text::normalize_transcript;

pub fn spawn(
    hotkey_rx: Receiver<HotkeyEvent>,
    cmd_rx: Receiver<CoreCmd>,
    ev_tx: Sender<CoreEvent>,
    settings: SharedSettings,
) {
    thread::Builder::new()
        .name("yap-core".into())
        .spawn(move || run(hotkey_rx, cmd_rx, ev_tx, settings))
        .expect("spawn core");
}

fn run(
    hotkey_rx: Receiver<HotkeyEvent>,
    cmd_rx: Receiver<CoreCmd>,
    ev_tx: Sender<CoreEvent>,
    settings: SharedSettings,
) {
    let mut stt: Option<Stt> = None;
    let mut recorder: Option<Recorder> = None;
    let mut history = match HistoryStore::open() {
        Ok(store) => Some(store),
        Err(e) => {
            let _ = ev_tx.send(CoreEvent::Log(format!("history db failed: {e}")));
            tracing::warn!("history db failed: {e:#}");
            None
        }
    };
    let vu_tick = tick(Duration::from_millis(50));

    try_load_stt(&mut stt, &ev_tx);
    publish_history(history.as_ref(), &ev_tx);

    loop {
        select! {
            recv(hotkey_rx) -> ev => {
                let Ok(ev) = ev else { break };
                match ev {
                    HotkeyEvent::Pressed => {
                        if stt.is_none() {
                            let _ = ev_tx.send(CoreEvent::Log("model not loaded — open Settings".into()));
                            let _ = ev_tx.send(CoreEvent::PhaseChanged(Phase::NeedsModel));
                            continue;
                        }
                        match Recorder::start() {
                            Ok(r) => {
                                recorder = Some(r);
                                let _ = ev_tx.send(CoreEvent::PhaseChanged(Phase::Recording));
                            }
                            Err(e) => {
                                let _ = ev_tx.send(CoreEvent::Log(format!("mic error: {e}")));
                            }
                        }
                    }
                    HotkeyEvent::Released => {
                        let Some(rec) = recorder.take() else { continue };
                        let samples = rec.stop();
                        let _ = ev_tx.send(CoreEvent::Level(0.0));
                        let current_settings = settings.read().clone();
                        if samples.len() < current_settings.min_samples {
                            let _ = ev_tx.send(CoreEvent::PhaseChanged(Phase::Idle));
                            continue;
                        }
                        let _ = ev_tx.send(CoreEvent::PhaseChanged(Phase::Transcribing));
                        let Some(s) = stt.as_mut() else { continue };
                        let raw_text = s.transcribe(&samples);
                        let output_text = normalize_transcript(&raw_text);
                        if let Some(history) = history.as_mut() {
                            if let Err(e) = history.save_recording(&samples, &raw_text, &output_text) {
                                let _ = ev_tx.send(CoreEvent::Log(format!("history save failed: {e}")));
                                tracing::warn!("history save failed: {e:#}");
                            }
                        }
                        publish_history(history.as_ref(), &ev_tx);
                        if !output_text.is_empty() {
                            let mut pasted_text = output_text.clone();
                            if current_settings.trailing_space { pasted_text.push(' '); }
                            if let Err(e) = paste_text(&pasted_text) {
                                let _ = ev_tx.send(CoreEvent::Log(format!("paste failed: {e}")));
                            }
                            let _ = ev_tx.send(CoreEvent::Transcript(output_text));
                        }
                        let _ = ev_tx.send(CoreEvent::PhaseChanged(Phase::Idle));
                    }
                }
            }
            recv(cmd_rx) -> ev => {
                let Ok(ev) = ev else { break };
                match ev {
                    CoreCmd::StartDownload => spawn_download(ev_tx.clone()),
                    CoreCmd::ReloadModel => {
                        stt = None;
                        try_load_stt(&mut stt, &ev_tx);
                    }
                }
            }
            recv(vu_tick) -> _ => {
                if let Some(r) = recorder.as_ref() {
                    let _ = ev_tx.send(CoreEvent::Level(r.current_level()));
                }
            }
        }

        let _ = settings.read().save();
    }
}

fn publish_history(history: Option<&HistoryStore>, ev_tx: &Sender<CoreEvent>) {
    let Some(history) = history else {
        return;
    };
    match history.recent(5) {
        Ok(rows) => {
            let _ = ev_tx.send(CoreEvent::History(rows));
        }
        Err(e) => {
            let _ = ev_tx.send(CoreEvent::Log(format!("history load failed: {e}")));
            tracing::warn!("history load failed: {e:#}");
        }
    }
}

fn try_load_stt(stt: &mut Option<Stt>, ev_tx: &Sender<CoreEvent>) {
    let _ = ev_tx.send(CoreEvent::PhaseChanged(Phase::LoadingModel));
    let paths = ModelPaths::new(parakeet_dir());
    tracing::info!("checking model at {}", paths.root.display());
    if !paths.is_complete() {
        tracing::warn!(
            "model incomplete (encoder={}, decoder={}, joiner={}, tokens={})",
            file_meta(&paths.encoder()),
            file_meta(&paths.decoder()),
            file_meta(&paths.joiner()),
            file_meta(&paths.tokens()),
        );
        let _ = ev_tx.send(CoreEvent::PhaseChanged(Phase::NeedsModel));
        return;
    }
    match Stt::load(&paths) {
        Ok(s) => {
            let mut s = s;
            let load_log = match s.self_test(&paths) {
                Ok(Some(text)) if text.is_empty() => {
                    tracing::warn!("Parakeet self-test produced an empty transcript");
                    "Parakeet loaded, but bundled test audio produced no transcript".into()
                }
                Ok(Some(text)) => {
                    tracing::info!("Parakeet self-test transcript: {text}");
                    "Parakeet model loaded".into()
                }
                Ok(None) => "Parakeet model loaded".into(),
                Err(e) => {
                    tracing::warn!("Parakeet self-test failed: {e:#}");
                    format!("Parakeet loaded, but model self-test failed: {e}")
                }
            };
            *stt = Some(s);
            let _ = ev_tx.send(CoreEvent::PhaseChanged(Phase::Idle));
            let _ = ev_tx.send(CoreEvent::Log(load_log));
            tracing::info!("Parakeet loaded from {}", paths.root.display());
        }
        Err(e) => {
            tracing::error!("Stt::load failed: {e:#}");
            let _ = ev_tx.send(CoreEvent::Log(format!("model load failed: {e}")));
            let _ = ev_tx.send(CoreEvent::PhaseChanged(Phase::NeedsModel));
        }
    }
}

fn file_meta(p: &std::path::Path) -> String {
    match std::fs::metadata(p) {
        Ok(m) => format!("{} bytes", m.len()),
        Err(_) => "missing".into(),
    }
}

fn spawn_download(ev_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let _ = ev_tx.send(CoreEvent::DownloadFailed(e.to_string()));
                return;
            }
        };
        let sink_tx = ev_tx.clone();
        let sink: ProgressSink = Arc::new(move |p: Progress| match p {
            Progress::Downloading { bytes, total } => {
                let _ = sink_tx.send(CoreEvent::DownloadProgress { bytes, total });
            }
            Progress::Extracting => {
                let _ = sink_tx.send(CoreEvent::DownloadExtracting);
            }
            Progress::Done => {
                let _ = sink_tx.send(CoreEvent::DownloadDone);
            }
        });

        let result: Result<_> = rt.block_on(download_parakeet(sink));
        match result {
            Ok(_) => {
                let _ = ev_tx.send(CoreEvent::DownloadDone);
            }
            Err(e) => {
                let _ = ev_tx.send(CoreEvent::DownloadFailed(e.to_string()));
            }
        }
    });
}
