<div align="center">
  <img src="assets/icons/yap.svg" alt="Yap" width="128" height="128" />

  # Yap

  **Push-to-talk dictation for macOS. Offline. Instant.**

  Hold a hotkey, speak, release — Yap pastes the transcript into whatever app you're in.

  [![Release](https://img.shields.io/github/v/release/lassejlv/yap?label=download&color=ff5e5e)](https://github.com/lassejlv/yap/releases/latest)
  [![Build](https://github.com/lassejlv/yap/actions/workflows/release.yml/badge.svg)](https://github.com/lassejlv/yap/actions/workflows/release.yml)
</div>

---

## Features

- **Offline ASR** — NVIDIA Parakeet TDT 0.6B v2 (int8) running locally via [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx). No network round-trip, no cloud, no logs leaving your machine.
- **Push-to-talk** — global `CGEventTap` watches a configurable modifier; press to record, release to transcribe and paste.
- **Menubar app** — no Dock icon (`LSUIElement`), settings window on demand.
- **Transcript history** — local SQLite, browsable from Settings.
- **Native UI** — built on [GPUI](https://www.gpui.rs/) (Zed's renderer) with [gpui-component](https://github.com/longbridge/gpui-component).

## Install

Grab the DMG that matches your Mac from [the latest release](https://github.com/lassejlv/yap/releases/latest):

| Mac | Asset |
|---|---|
| Apple Silicon (M1/M2/M3/M4) | `Yap-<version>-macos-aarch64.dmg` |
| Intel | `Yap-<version>-macos-x86_64.dmg` |

Open the DMG → drag **Yap** into Applications → launch.

> The build is ad-hoc codesigned, not Apple-notarised. macOS will show a Gatekeeper warning the first time. Right-click the app → **Open** → **Open** to bypass.

### Permissions

On first launch, macOS will prompt you to grant:

1. **Microphone** — required to record audio.
2. **Input Monitoring** *and* **Accessibility** — required for the global push-to-talk hotkey and to paste the transcript into the focused app.

Both prompts come from System Settings → Privacy & Security.

### First-run model download

Yap pulls the Parakeet TDT v2 model (~600 MB) on first launch into:

```
~/Library/Application Support/yap/models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8/
```

If you previously had the `whispr/` data directory, Yap renames it to `yap/` automatically so the model and history carry over.

## Usage

1. Open **Settings → Hotkey** and pick a push-to-talk modifier (defaults to ⌃ Right Control).
2. Focus any text field in any app.
3. Hold the hotkey, speak, release. The transcript pastes at the cursor.

Transcript history is in **Settings → History**.

## Build from source

Requires Rust (stable, edition 2024) and macOS 12+.

```bash
git clone https://github.com/lassejlv/yap.git
cd yap
cargo run --release
```

### Build a distributable DMG

```bash
./scripts/build-dmg.sh
# → dist/Yap-<version>.dmg
```

The script:

1. `cargo build --release`
2. Assembles `Yap.app` (binary, `Info.plist`, icon)
3. Bundles `libsherpa-onnx-c-api.dylib` + `libonnxruntime.dylib` into `Contents/Frameworks/` with `@executable_path/../Frameworks` rpath
4. Rasterises `assets/icons/yap.svg` into a multi-resolution `.icns` (uses macOS-builtin `qlmanage`, `sips`, `iconutil` — no Homebrew dependency)
5. Ad-hoc codesigns
6. Wraps everything in a UDZO `.dmg` with an `/Applications` symlink for drag-to-install

## Release flow

Tags must equal the `Cargo.toml` version (stripping any leading `v`):

```bash
# bump Cargo.toml → 0.2.0, commit
git tag v0.2.0
git push origin v0.2.0
gh release create v0.2.0 --title "Yap 0.2.0" --notes "..."
```

Publishing the release fires `.github/workflows/release.yml`, which:

- Verifies `Cargo.toml` version matches the tag (fails the run otherwise).
- Builds natively on `macos-14` (aarch64) **and** `macos-13` (x86_64).
- Sanity-checks the resulting bundle (binary, plist, icns, dylibs, rpath).
- Attaches `Yap-<version>-macos-<arch>.dmg` + `.sha256` sidecars to the release.

## Architecture

```
                 ┌───────────────────────┐
   CGEventTap ──▶│  hotkey thread        │── HotkeyEvent ──┐
                 └───────────────────────┘                 │
                                                           ▼
                                            ┌───────────────────────────┐
                              UiCmd ───────▶│   core thread             │
                                            │   (state machine)         │
                                            └─────┬────────────┬────────┘
                                                  │            │
                                                  ▼            ▼
                                          cpal capture    sherpa-onnx STT
                                                  │            │
                                                  └────┬───────┘
                                                       ▼
                                              text normalize + paste
                                              (arboard + AppleEvents)
```

| Module | Job |
|---|---|
| `src/hotkey.rs` | `CGEventTap` flags-changed watcher → `HotkeyEvent { Pressed, Released }` |
| `src/audio.rs` | `cpal` mic capture, 16 kHz mono PCM ring |
| `src/stt.rs` + `src/model.rs` | sherpa-onnx Parakeet wrapper + model download |
| `src/text.rs` | Spoken-word normalisation (e.g. "dash dash release" → `--release`) |
| `src/paste.rs` | Clipboard + simulated ⌘V paste |
| `src/history.rs` | SQLite transcript log |
| `src/ui.rs` + `src/ui/` | GPUI Settings window |
| `src/tray.rs` | `NSStatusBar` menubar item |
| `src/core.rs` | Owns audio + STT, coordinates threads |

## Acknowledgements

- [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) — the inference runtime
- [NVIDIA Parakeet](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2) — the model
- [Zed Industries / GPUI](https://github.com/zed-industries/zed) — the renderer
- [gpui-component](https://github.com/longbridge/gpui-component) — UI primitives

## License

TBD — add a `LICENSE` file before public distribution.
