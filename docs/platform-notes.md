# Platform notes

What was tried, what the OS did, and what it cost. Written down as it happens —
the same problems come back at M7 during signing, and nobody remembers by then.

Newest first.

---

## M2 — hotkey and injection

### Passed on Windows

Dictation lands in Notepad, a browser text field, VS Code and a messenger. The
clipboard comes back afterwards. A foreground window running elevated is
reported as such instead of the paste vanishing — the integrity-level check does
what it was written to do, which was the part of this milestone written blind
and least certain.

### Measured, on the target machine

Key-up to inserted text, `dictate` on the same RTX 5070 Ti:

| | Transcribe | Inject | Key-up → text |
|---|---|---|---|
| Language auto-detected | 385 ms | 5 ms | 423 ms |
| `--language ru` | 271 ms | 5 ms | **315 ms** |

Injection is 5 ms against a 50 ms budget — the clipboard path costs almost
nothing, and the wait to restore happens on another thread after the text has
already landed.

The 114 ms difference is whisper's language detector, exactly as it was in M1.
The app must pin the language once the user has chosen one; leaving it on auto
spends a quarter of the transcription budget deciding something the user
already knows.

Both numbers are still record-then-transcribe. M3's sliding window only has to
finish the tail after key-up, so there is room.

### The hook callback is on the system's critical path

`WH_KEYBOARD_LL` callbacks run on the thread that installed the hook, and while
one is running *no keystroke on the machine is delivered to anybody*. Windows
enforces this with `LowLevelHooksTimeout` (300 ms by default): exceed it and the
hook is silently removed — no error, the hotkey simply stops working.

So the callback compares a virtual key, reads modifier state, and pushes onto a
channel. Nothing else. The caller's closure runs on a separate dispatch thread.
For the same reason it uses `try_borrow_mut` rather than `borrow_mut`: a panic
would unwind into a Windows callback, which is undefined behaviour.

### Key repeat, and releasing the modifier first

Holding a key produces a stream of `WM_KEYDOWN`, so the hook tracks whether it
is already in a hold and reports only the first. On the way up it deliberately
does *not* check modifiers — releasing Ctrl a moment before Space is normal, and
the dictation still has to end.

The trigger is swallowed (return 1) only when the full binding matched, so
dictating into an editor does not also type a space into it. An unmatched Space
is passed through, which matters rather a lot.

### Restoring the clipboard is best-effort, and says so

`GetClipboardData` hands back a handle the clipboard owns. For most formats that
handle is global memory and can be copied; for `CF_BITMAP` and anything the
owner renders on demand it is not, and `GlobalSize` returning 0 is how we tell.
Those formats cannot be restored. `Snapshot::is_complete` reports it rather than
letting the user find out, and the app should surface it.

The restore itself runs on a background thread after 150 ms: the paste is
asynchronous, and taking the clipboard back before the target has read it would
paste the user's old contents instead of their dictation.

### Elevation

`OpenProcess` with `PROCESS_QUERY_LIMITED_INFORMATION` failing with
`ERROR_ACCESS_DENIED` already answers the question — a medium-integrity process
cannot query an elevated one. Where it succeeds, the integrity levels are
compared directly via the token's mandatory label. Unknown is treated as "try
anyway": refusing to dictate on a maybe would be worse than a paste that does
not land.

---

## M1 — speech to text

### Measured, on the target machine

RTX 5070 Ti (Blackwell, sm_120), CUDA 13.3, `large-v3-turbo-q5_0`, whisper.cpp
1.8.3, greedy sampling with the language pinned:

| Audio | Transcription | Real time |
|---|---|---|
| 4.98 s (en) | 259 ms | 19.2× |
| 4.98 s (ru) | 270 ms | 18.4× |
| 9.98 s (ru) | 320 ms | 31.1× |

The M1 criterion was a 10-second clip under one second; 320 ms clears it three
times over. Model load is 610-630 ms and is *not* part of that — the app loads
once at startup and keeps the model resident, which is what M3 depends on.

Note that transcription time barely doubles as the audio doubles: the encoder
runs over a fixed 30-second window regardless, so most of the cost is constant.
That is good news for M3's sliding window and bad news for anyone hoping short
utterances would be proportionally cheaper.

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

### Silence does not come back as an empty transcript

Whisper given near-silence does not return "". It invents a plausible sentence,
and with the language pinned it will still reach for another alphabet — a five
second silent take came back as `Д 새로운 городской грани Русанови`. A test
dictation that produced nothing therefore looks identical to one that produced
nonsense, and the natural conclusion is that the model or the GPU is broken.

`klar-cli` now prints SPEAK NOW, shows a live level meter while recording, and
refuses to transcribe a buffer whose peak is under 0.01, naming the device
instead. The app will need the same guard on the real path.

### LNK4098: LIBCMT conflicts

The CUDA build links objects compiled against the static CRT alongside Rust's
dynamic one. The warning is benign in practice — noted here because a genuine
CRT mismatch shows up much later as corruption around allocations, and it would
be a shame to rediscover this line then.

### Four build prerequisites, none of which name themselves

Building `whisper-rs` on Windows needs four things beyond the Rust toolchain,
and each one missing fails in a way that does not mention what is missing:

| Missing | What you see |
|---|---|
| CUDA Toolkit | `called \`Result::unwrap()\` on an \`Err\` value: NotPresent` from `whisper-rs-sys/build.rs:59` — that is `env::var("CUDA_PATH")` |
| LLVM | `Unable to find libclang` from `bindgen`, which generates the whisper.cpp bindings |
| CMake | `failed to execute command: program not found` from `cmake-0.1.58` |
| MSVC Build Tools | link errors |

They surface strictly one at a time — each fix reveals the next — so the first
looks like the whole problem. The README now lists all four up front.

Two things worth knowing. Installing any of them does not help the shell you are
already in: the environment variables only reach new processes. And Visual
Studio's C++ workload *does* ship CMake, but under
`Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin` rather than on
`PATH`, so cargo cannot see it — install CMake standalone.

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
