# Platform notes

What was tried, what the OS did, and what it cost. Written down as it happens —
the same problems come back at M7 during signing, and nobody remembers by then.

Newest first.

---

## M1 — speech to text

### The cross-target check stopped working, and that is fine

M0 verified `src-tauri` from Linux with `cargo check --target
x86_64-pc-windows-msvc`. That no longer covers the whole workspace: `whisper-rs`
builds whisper.cpp through `cmake`, which needs a real MSVC toolchain rather
than just the Rust target. Windows verification now rests on the
`windows-latest` CI job, which builds the workspace for real.

### The auto-detect pass is not free

Transcribing an 11-second clip with `tiny.en` took 3109 ms; pinning the language
took it to 1617 ms. Whisper runs its language detector over the first window
whenever the language is `None` — and on an English-only model the answer it
returns is noise (`sq`, p = 0.01). `WhisperTranscriber` now pins `en` for any
model that reports `is_multilingual() == false`, and the app should pin the
language for multilingual models too once the user has chosen one.

### Proving the backend, not assuming it

Compile-time features say what was *asked* for. The runtime answer comes from
whisper.cpp itself: `install_logging_hooks()` routes its device-registration
lines into `tracing`, so `whisper_backend_init_gpu: no GPU found` lands in
Klar's log. `whisper_print_system_info()` (via the `raw-api` feature) adds the
accelerator list the binary actually carries. `klar-cli doctor` prints both.

No GPU feature is on by default: CI has no CUDA toolkit, and a default that
silently produced a CPU build is exactly the failure this milestone exists to
rule out.

### The CUDA build fails with a bare `unwrap` if the toolkit is missing

`whisper-rs-sys`'s build script reads `CUDA_PATH` and unwraps it
(`build.rs:59`), so a machine without the CUDA Toolkit — or a shell opened
before it was installed — gets:

```
called `Result::unwrap()` on an `Err` value: NotPresent
```

with no mention of CUDA. It is worth recognising on sight: install the CUDA
Toolkit, open a new shell so `CUDA_PATH` is in the environment, and rebuild.
`nvcc --version` is the quick check.

Note that a CPU build exercises everything in M1 except the latency budget, so
capture, model download and transcription accuracy can all be verified while
the toolkit downloads.

### cpal 0.18.1 can resolve itself into a build failure on Windows

cpal declares `windows` and `windows-core` as two *independent* version ranges,
both `>=0.61, <=0.62`. Either matched pair compiles. The mix does not — and
cargo is free to pick `windows 0.61.3`, which carries `windows-core 0.61.2`,
alongside a separate `windows-core 0.62.2`. Two `windows_core` crates in one
graph, and cpal's own `#[windows::core::implement]` macro then fails with
eighteen errors about "multiple different versions of crate `windows_core`",
none of which mention cpal's manifest.

`klar-core` now names both crates in a `cfg(target_os = "windows")` dependency
block to hold them to one major. They are not used directly; the block exists
only so the resolver cannot roll the broken combination. Verified by deleting
`Cargo.lock`, resolving from scratch and running `cargo check -p cpal --target
x86_64-pc-windows-msvc`.

Worth knowing that a committed `Cargo.lock` is not protection here: ours had the
broken mix in it, which is how it reached a developer machine.

### Linux needs ALSA headers

`cpal` will not build without `libasound2-dev`. Linux is not a target, but the
pipeline crates are developed and unit-tested there, so the Ubuntu CI job
installs it.

### Model integrity

HuggingFace's `x-linked-etag` header carries the file's SHA-256 (it is the LFS
oid), which is where the catalogue's checksums come from — no need to download
half a gigabyte to record a hash. Resume uses a `.part` file and a `Range`
request; a server that answers 200 to a ranged request is refused outright
rather than appended to, because appending would produce a corrupt model that
fails at inference time instead of at load time.

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
