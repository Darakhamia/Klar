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

**M0 — scaffold.** The workspace, the state machine, the platform traits, the
Tauri shell and CI are in place. There is no audio yet: the platform backends
return `NotImplemented` and the window reports what the Rust side knows about
itself. M1 is speech to text, headless.

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
npm run tauri dev      # the app
cargo run -p klar-cli -- doctor    # what the OS says about permissions
cargo run -p klar-cli -- dry-run   # the event stream the overlay will render
```

Before every commit:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run lint
```

## Licence

MIT.
