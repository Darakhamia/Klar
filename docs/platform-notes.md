# Platform notes

What was tried, what the OS did, and what it cost. Written down as it happens —
the same problems come back at M7 during signing, and nobody remembers by then.

Newest first.

---

## M0 — scaffold

### Cross-compiling the check, not the build

The workspace is developed on Linux but targets Windows. `cargo check --target
x86_64-pc-windows-msvc` gets the whole tree — including `src-tauri`, `tao` and
`webview2-com` — type-checked without an MSVC linker, because `check` never
links. It emits `warning: GNU compiler is not supported for this target` from a
build script; that is the `cc` crate noticing the host compiler and is harmless
for a check.

Building for real still needs a Windows machine or the `windows-latest` CI
runner.

### Linux is not a target, but the crates must build there

`klar-core`, `klar-platform` and `klar-cli` compile and test on Linux so the
pipeline can be developed off a target machine. `klar-platform` therefore has an
`unsupported` backend whose every call returns `PlatformError::NotImplemented`
rather than a silent no-op. `src-tauri` does not build on Linux at all (no
`gdk-3.0`), which is fine — it is never run there.

### Icons

Generated from the Modernist tokens by a small stdlib script (no image library
was available): red waveform bars on ink, zero radius. The `.ico` uses PNG
payloads, which Windows has accepted since Vista. `.icns` is not generated —
it is only needed once macOS bundling starts.

### Frontend fonts are vendored

The design calls for Archivo everywhere. Loading it from Google Fonts would mean
a network request from a desktop app that promises nothing leaves the machine,
and the CSP in `tauri.conf.json` blocks it anyway. `@fontsource/archivo` puts the
woff2 files in the bundle instead.

---

## Ahead of us

Recorded now so they are not rediscovered under time pressure:

- **`WH_KEYBOARD_LL` needs its own thread with a message pump.** The callback
  runs on that thread and blocks the system input queue while it does; anything
  slow or panicking in there stalls the whole machine.
- **`RegisterHotKey` is not usable.** It reports press only. Hold-to-talk needs
  key-up, so the low-level hook is the only path.
- **Windows elevation.** Injecting into an elevated window from a non-elevated
  process fails silently. Compare integrity levels first and return
  `PlatformError::ElevatedTarget` rather than appearing to hang.
- **macOS Accessibility is bound to the code signature.** Ad-hoc signing with a
  stable identifier has to be in place before the macOS backend is touched, or
  the grant is lost on every rebuild.
- **macOS `fn` key.** It never reaches a `CGEventTap`; binding it needs an
  `NSEvent` global monitor.
