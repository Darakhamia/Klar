# CLAUDE.md

Project context for Claude Code. Read this before touching anything.

## What we are building

**Klar** — a desktop voice dictation app for macOS (Apple Silicon) and Windows. It sits in the menu bar / system tray. The user holds a hotkey anywhere in the OS, speaks, releases, and clean written text is inserted at the cursor in whatever app is focused.

It is not a transcriber. Raw speech is transcribed, then polished by an LLM: filler words removed, punctuation fixed, self-corrections resolved ("Tuesday — no, Friday" → "Friday"), lists formatted. Messy speech in, finished text out.

Speech is processed locally by default. Cloud is an opt-in fallback, never the default.

## Platform order

**Windows ships first.** macOS is still a target and its constraints still shape
every trait in `klar-platform` — the backend exists, compiles, and is in CI from
M0 so it cannot silently rot — but it stays a stub until Windows reaches M7.

Read the milestone acceptance criteria in `docs/BUILD-PLAN.md` as "on Windows"
until then. The rule they are protecting still holds: do not weaken a criterion
to get past it.

## The one metric that matters

Perceived latency. From key release to text on screen:

| Stage | Budget |
|---|---|
| Final transcription after key release | ≤ 400 ms |
| LLM polish | ≤ 400 ms |
| Injection | ≤ 50 ms |

If a design choice makes the app prettier or more general but blows this budget, the budget wins. Transcription runs **during** speech on a sliding window — do not wait for the key release to start ASR.

## Stack

- **Shell:** Tauri v2 — Rust backend, web frontend. Chosen over Electron because the app is resident all day and must stay under ~40 MB RSS.
- **Frontend:** React + TypeScript + Vite. Settings, onboarding, dictionary, history, stats only. No business logic in the frontend.
- **Audio capture:** `cpal`, resampled to 16 kHz mono with `rubato`.
- **VAD:** Silero, through whisper.cpp's own VAD rather than a separate ONNX
  runtime — same ggml backend, no second inference stack in the installer. See
  `docs/platform-notes.md`.
- **ASR:** `whisper-rs` (bindings to whisper.cpp). `metal` + `coreml` features on macOS, `cuda` on Windows, CPU fallback everywhere. Model: `large-v3-turbo`, q5_0.
- **Polish:** trait-based. Local Ollama over HTTP first; cloud (Groq) behind the same trait.
- **Storage:** `rusqlite` with the `bundled` feature.
- **Async:** `tokio`.
- **Errors:** `thiserror` in library crates, `anyhow` at the application boundary.
- **Logging:** `tracing` + `tracing-subscriber`, writing to a rolling file in the app data dir.

Do not pin versions from memory. Check the current version of every crate before adding it.

## Workspace layout

```
klar/
├── crates/
│   ├── klar-core/      # no Tauri, no UI — the entire pipeline
│   ├── klar-platform/  # OS-specific: hotkey hooks, text injection, permissions
│   └── klar-cli/       # headless binary for testing the pipeline
├── src-tauri/          # thin Tauri wrapper: commands, tray, windows, IPC
├── src/                # React frontend
├── design/             # the design output — tokens, overlay, onboarding, windows
├── docs/               # build plan, platform notes
└── models/             # downloaded at runtime, gitignored
```

`klar-core` must compile and be testable without Tauri. If a feature cannot be exercised from `klar-cli`, it is in the wrong crate.

## Pipeline

```
hotkey down
  → capture (cpal, 16 kHz mono)
  → ring buffer + VAD
  → streaming ASR (partial results)
hotkey up
  → finalize ASR
  → dictionary substitution
  → LLM polish
  → inject text
  → persist to SQLite
```

State machine: `Idle → Recording → Transcribing → Polishing → Injecting → Idle`, plus `Error` reachable from any state. Every transition emits an event the UI subscribes to; the overlay renders purely from this state.

## Platform gotchas — read before writing platform code

**Hold-to-talk is the hard part.** `tauri-plugin-global-shortcut` fires on press and does not give reliable key-up. Push-to-talk needs raw hooks: `CGEventTap` on macOS, `WH_KEYBOARD_LL` on Windows. Isolate this in `klar-platform` behind one trait. If the user binds `fn` on macOS, that key does not appear in a normal event tap — it needs an `NSEvent` global monitor.

**macOS Accessibility permission.** Required for both the event tap and text injection. It is bound to the code signature: an unsigned dev build loses the granted permission on every rebuild. Set up ad-hoc signing with a stable identifier early or debugging becomes miserable.

**Text injection.** Primary path is clipboard: save the existing clipboard, set our text, synthesize Cmd+V / Ctrl+V, restore the previous clipboard after a short delay. This is the only approach that reliably handles Unicode, emoji, and long text. Keep synthesized keystrokes (`CGEventPost` / `SendInput`) as a fallback for apps that block programmatic paste. Never lose the user's clipboard contents — restore it even on the error path.

**Windows elevation.** Injection into an elevated window from a non-elevated process silently fails. Detect it and surface a real error rather than appearing to hang.

## Conventions

- No `unwrap()` or `expect()` in `klar-core` or `klar-platform`. Tests and `main` are fine.
- No `panic!` on any path reachable from audio or hotkey handling — a crash there kills a background app the user cannot see.
- Frontend talks to Rust only through named Tauri commands and events. No logic duplicated in TypeScript.
- The design tokens in `src/styles/tokens.css` come from the design phase. Use them; do not invent colors or type sizes.
- Commit per working increment, not per file. Run `cargo clippy --all-targets -- -D warnings` and `cargo fmt` before each commit.
- Anything touching the microphone, the clipboard, or the network gets a unit test or an explicit note explaining why it cannot have one.

## Privacy rules — non-negotiable

- Audio is never written to disk except in an explicit debug mode that is off by default.
- Nothing leaves the machine unless the user has switched a specific step to a cloud provider, and that switch is visible in the UI while it is active.
- History is stored locally. There is no telemetry, no analytics SDK, no crash reporter phoning home in v1.

## Out of scope for v1

Command mode (editing selected text by voice), mobile, Linux, team features, cloud sync. Do not build scaffolding for them.
