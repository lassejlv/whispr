# whispr

Whispr Flow clone for macOS. Hold **Fn**, speak, release — cleaned dictation lands in the focused app.

## Stack

- Rust + [gpui](https://github.com/zed-industries/zed) + [gpui-component](https://longbridge.github.io/gpui-component/)
- STT backends:
  - **OpenAI Cloud** — `gpt-4o-transcribe` (default, requires API key)
  - **Parakeet Local** — NVIDIA Parakeet TDT v2 via [`scriptrs`](https://github.com/avencera/scriptrs), native CoreML on Apple Silicon, no network after first run
- `gpt-4o-mini` cleanup pass (optional, requires OpenAI key)
- `cpal` for capture, `CGEventTap` for the Fn key, `arboard` / `enigo` for output

## Run

```bash
cp .env.example .env
# put your OpenAI key in .env
just run
```

On first launch macOS will ask for **Microphone** and **Accessibility** permission. Both required.

## Settings

Menubar → Open Settings. Backend (OpenAI / Parakeet), API key, output mode (paste vs type), cleanup prompt, custom vocabulary. Persisted to `~/Library/Application Support/dev.whispr.whispr/config.toml`. Recordings stored to `whispr.db` + `recordings/*.wav` in the same directory.

### Offline mode (Parakeet)

Switch the backend to **Parakeet** in Settings. First dictation triggers a ~600 MB download of the Parakeet TDT v2 CoreML bundle from Hugging Face (`avencera/scriptrs-models`). Subsequent runs are fully offline. Override the model path via `SCRIPTRS_MODELS_DIR=/path/to/models`.

## Commands

```
just         # run release build
just dev     # run debug with verbose logs
just check
just lint
just fmt
```
