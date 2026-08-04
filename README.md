# Klar

Messy speech in, finished text out.

Hold a hotkey anywhere in the OS, speak, release — clean written text appears at
the cursor in whatever application is focused. Speech is transcribed locally and
then polished: fillers removed, punctuation fixed, spoken self-corrections
resolved. Nothing leaves the machine unless you switch a specific step to a
cloud provider, and that switch is visible while it is active.

Windows first; macOS follows. See [`CLAUDE.md`](CLAUDE.md) for the architecture
and [`docs/BUILD-PLAN.md`](docs/BUILD-PLAN.md) for the milestones.

## Status

**M1 — speech to text, headless.** Capture, resampling, model download and
whisper.cpp all work from `klar-cli`; the GPU build is verified per machine, not
assumed. No hotkey and no injection yet — those are M2, and the platform
backends still return `NotImplemented`.

## Layout

```
crates/klar-core/      the entire pipeline — no Tauri, no UI
crates/klar-platform/  hotkey hooks, text injection, permissions
crates/klar-cli/       headless harness; every risky thing is spiked here first
src-tauri/             thin Tauri wrapper: commands, tray, windows, IPC
src/                   React frontend
design/                the design output the interface is built from
```

## Getting started

Requires [Rust](https://rustup.rs) (1.85+), Node 22, and — on Windows —
[WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) (already
present on Windows 11) plus the MSVC build tools.

```sh
npm install
npm run tauri dev                  # the app
cargo run -p klar-cli -- doctor    # OS, audio devices, ASR backend, models
```

### Dictating from the command line

whisper.cpp is built from source, so the first `cargo build` after a clean
checkout takes a few minutes and needs `cmake` on the PATH.

```sh
# Build with the GPU your machine actually has. Without a feature flag you get
# a CPU build, which works and misses the latency budget by an order of
# magnitude — deliberately loud rather than silent.
cargo build -p klar-cli --features cuda      # Windows, NVIDIA
cargo build -p klar-cli --features metal     # macOS

cargo run -p klar-cli --features cuda -- model download
cargo run -p klar-cli --features cuda -- listen --seconds 5
```

`doctor` prints the backend and `listen` prints the real-time factor, so a
build that quietly fell back to CPU is visible immediately rather than at M3.

Other commands: `devices`, `record --seconds 5 --out debug.wav`,
`transcribe file.wav`, `model list|verify`, `dry-run`.

Set `KLAR_MODELS_DIR` to keep models out of the app data directory.

Before every commit:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run lint
```

## Licence

MIT.
