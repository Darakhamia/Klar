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

**M6 in progress — the app.** Tray icon, a dictation engine running the M1-M3
pipeline in the background, the overlay from the design (direction A,
"Collapse"), and the settings window: General and Voice are live and take
effect immediately; Dictionary, History and Stats are waiting on the database
in M5 and say so rather than showing invented rows. Onboarding is next.

**M3 passed on Windows.** Transcription runs during speech and commits at
pauses, so key release leaves only the tail: median 260 ms from key-up to
inserted text against a 500 ms criterion. A 22-second dictation streams 21.8 s
of it while the user is still talking.

**M2 passed on Windows.** Hold Ctrl+Space anywhere, speak, release, and the
text appears at the cursor — 315 ms from key-up, injection 5 ms of that.
Verified into Notepad, a browser field, VS Code and a messenger; the clipboard
survives; an elevated foreground window is reported rather than silently
swallowing the paste.

**M1 passed on Windows.** Capture, resampling, model download and whisper.cpp
work from `klar-cli` on a real machine: 9.98 s of speech transcribed in 320 ms
on CUDA, against a criterion of one second. See `docs/platform-notes.md` for the
numbers. No hotkey and no injection yet — those are M2, and the platform
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

Requires [Rust](https://rustup.rs) (1.85+) and Node 22.

### Windows prerequisites

whisper.cpp is compiled from source and its bindings are generated at build
time, so more than the Rust toolchain is needed. Install all of these before
the first build — each one missing produces an error that does not name it.

| What | Why | Install |
|---|---|---|
| MSVC Build Tools (Desktop C++) | The compiler and linker | `winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"` |
| CMake | whisper.cpp is a CMake project. Visual Studio bundles a copy, but only inside its own directory — it is not on `PATH`, so install it standalone | `winget install Kitware.CMake` |
| LLVM | `bindgen` needs `libclang.dll` to generate the whisper.cpp bindings | `winget install LLVM.LLVM` |
| CUDA Toolkit | Only for `--features cuda`. Blackwell (RTX 50xx) needs 12.8+ | `winget install Nvidia.CUDA` |
| WebView2 | The Tauri window. Already present on Windows 11 | [Download](https://developer.microsoft.com/microsoft-edge/webview2/) |

Open a new terminal afterwards: `CUDA_PATH` and the LLVM path only reach
processes started after installation. If `bindgen` still cannot find libclang,
point it at the install explicitly:

```powershell
[Environment]::SetEnvironmentVariable("LIBCLANG_PATH", "C:\Program Files\LLVM\bin", "User")
```

macOS needs Xcode command line tools and CMake (`brew install cmake`).

```sh
npm install
npm run tauri dev -- --features cuda   # the app, on the GPU
cargo run -p klar-cli -- doctor    # OS, audio devices, ASR backend, models
```

First launch opens onboarding: it tests the microphone, downloads the speech
model with a progress bar, and ends on a text box you dictate into. Nothing
below is needed to get the app working — the CLI is the harness the pipeline is
built and measured in.

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

### Hold to talk

```sh
cargo run -p klar-cli --features cuda -- hotkey     # does the hook see both edges?
cargo run -p klar-cli --features cuda -- inject "hello" --delay 3
cargo run -p klar-cli --features cuda -- dictate    # the whole loop
```

`dictate` loads the model once, then waits: hold Ctrl+Space anywhere, speak,
release, and the text lands at the cursor in whatever is focused. It prints the
time from key-up to inserted text, which is the number CLAUDE.md's budget is
about.

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
