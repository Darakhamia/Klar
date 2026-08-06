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

**Usable.** Hold the hotkey, speak, release, text appears. Rebinding, start with
Windows, where the finished text goes, light and dark, overlay position, and
onboarding all work. The polish stage is built and **off by default** — it needs
a local model server, and dictation without it inserts the transcript as
recognised.

**Dictionary, history and statistics work.** Teach Klar a name it mangles and it
spells it correctly from the next sentence on — biased into whisper's prompt
before decoding, and substituted afterwards for whatever the bias missed. The
editor previews the substitution live, so a new entry can be checked without
holding the hotkey and hoping. Every dictation is recorded locally, labelled
with the application it went into, and deleting is per row or all at once. The
totals survive clearing the history: the text is the private part, how much you
used the app is not.

Not built: the cloud path for machines without a usable GPU. It says so in the
interface rather than being offered and doing nothing.

**Not only NVIDIA.** The GPU backend is a compile-time choice, so there is a
CUDA build and a Vulkan one, and Vulkan covers AMD, Intel and NVIDIA alike. The
Vulkan installer carries no redistributable runtime at all, and **it has now
been installed and used on three machines by three people** — a desktop
RTX 5070 Ti, a laptop RTX 3060, and an AMD Radeon RX 7900 XT. Two of those had
never had a developer toolchain on them, which is M7's actual criterion, and the
AMD card is the one the Vulkan work existed for. What
mattered more than adding it was noticing when it is wrong: a CUDA build on a
machine with an AMD card starts, works, silently runs on the CPU and is ten
times slower. Klar reads ggml's device registry at load and names the device
that will do the work — in the log, in `klar-cli doctor`, and in Settings. See
[Which GPU build](#which-gpu-build).

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
| Vulkan SDK | Only for `--features vulkan`. `glslc` compiles whisper.cpp's shaders and `Lib\vulkan-1.lib` is what it links against. Set `VULKAN_SDK` yourself — see below | `winget install KhronosGroup.VulkanSDK` |
| WebView2 | The Tauri window. Already present on Windows 11 | [Download](https://developer.microsoft.com/microsoft-edge/webview2/) |

Open a new terminal afterwards: `CUDA_PATH` and the LLVM path only reach
processes started after installation.

If `bindgen` still cannot find libclang, point it at the install explicitly:

```powershell
[Environment]::SetEnvironmentVariable("LIBCLANG_PATH", "C:\Program Files\LLVM\bin", "User")
```

#### `VULKAN_SDK` is not set for you

`whisper-rs-sys` panics with *"Please install Vulkan SDK and ensure that
VULKAN_SDK env variable is set"*, and the first half is usually a red herring:
the SDK installed fine and nothing pointed at it. A `winget` install runs the
LunarG installer unattended, and unattended is exactly when it skips setting the
machine-wide variable. Check the disk rather than the message:

```powershell
Get-ChildItem C:\VulkanSDK | Select-Object -Last 1     # what is actually there

$sdk = "C:\VulkanSDK\1.4.350.0"                        # your version
[Environment]::SetEnvironmentVariable("VULKAN_SDK", $sdk, "User")
$env:VULKAN_SDK = $sdk                                 # and for this shell
```

The build needs `$VULKAN_SDK\Lib\vulkan-1.lib` to link and `$VULKAN_SDK\Bin\glslc.exe`
to compile the shaders, both of which it finds from that one variable. None of
it is needed to *run* Klar — see [Which GPU build](#which-gpu-build).

#### Reading a whisper.cpp build failure

whisper.cpp emits a few thousand lines of CMake policy warnings on every
configure, and a failure is one line somewhere inside them. The tail of the
output is never the cause — it is `cmake ... exited with code 1` and a Rust
panic from the `cmake` crate, which says nothing. Keep the whole log and pull
the cause out of it:

```powershell
npm run tauri build -- --features vulkan 2>&1 | Out-File C:\klar-build.log
Select-String -Path C:\klar-build.log -Context 0,4 `
  -Pattern "CMake Error|error [A-Z]+\d+|error C\d|exceeds the OS max path|fatal error" |
  Select-Object -First 20
```

Works for `cargo build` the same way. Match `CMake Error` as well as MSBuild's
`error MSBnnnn`: which one you get depends on the generator, and a pattern for
only one of them comes back empty on a build that plainly failed. `-Context 0,4`
matters too — CMake puts the diagnosis on the lines *after* the word "Error".

#### The Vulkan build needs Ninja on Windows

Build it with MSBuild and it fails deep inside a path nobody chose:

```
Path: cmTC_f87cd.dir\Debug\cmTC_f87cd.tlog\ParallelCustomBuild.write.1.tlog
exceeds the OS max path limit. The fully qualified file name must be less than
260 characters.
```

and, once shortened just enough to get past that, as the same tree failing to
find itself:

```
error MSB6003: The specified task executable "link.exe" could not be run.
System.IO.DirectoryNotFoundException: Could not find a part of the path
'...\vulkan-shaders-gen-prefix\src\vulkan-shaders-gen-build\CMakeFiles\
CMakeScratch\TryCompile-nt4stu\cmTC_78702.dir\Debug\cmTC_78702.tlog'
```

Nothing is wrong with Vulkan in either case — it is found, `glslc` is found,
every shader extension is supported. ggml builds its shader compiler as a
*nested* CMake project under `vulkan-shaders-gen-prefix\src\vulkan-shaders-gen-build\`,
MSBuild adds a `CMakeScratch\TryCompile-xxxxxx\cmTC_xxxxx.dir\Debug\
cmTC_xxxxx.tlog\` tree beneath that for each compiler probe, and the result sits
at the edge of what MSBuild handles. The CUDA build never gets near it — it has
no nested project.

Use Ninja. It replaces MSBuild for the whisper.cpp build, creates none of that
tree, and compiles considerably faster:

```powershell
winget install Ninja-build.Ninja
$env:CMAKE_GENERATOR = "Ninja"          # the cmake crate reads this
Remove-Item -Recurse -Force C:\kv       # see below
```

**Wipe the target directory when you change generator.** `whisper-rs-sys` names
its build directory after the feature set, not the generator, so Ninja arrives
at a `CMakeCache.txt` that MSBuild left behind and fails on a platform nobody
passed this time:

```
CMake Error at CMakeLists.txt:2 (project):
  Generator
    Ninja
  does not support platform specification, but platform
    x64
  was specified.
```

The `x64` is the cache's, from `-Ax64` on the earlier Visual Studio run.

A short `CARGO_TARGET_DIR` — `C:\kv` rather than `C:\dev\Klar\target` — buys
about a dozen characters, which is enough for a debug build and not for a
release one, where `release\` costs two more than `debug\`. Worth setting
anyway, since switching `--features` rebuilds whisper.cpp from scratch in a
shared target directory and one directory per backend saves that every swap. It
is not a substitute for Ninja.

Enabling Win32 long paths does not reliably help either: the limit being hit is
MSBuild's handling, not the filesystem's.

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
cargo build -p klar-cli --features cuda      # Windows, NVIDIA only
cargo build -p klar-cli --features vulkan    # Windows, any card — AMD, Intel, NVIDIA
cargo build -p klar-cli --features metal     # macOS

cargo run -p klar-cli --features cuda -- model download
cargo run -p klar-cli --features cuda -- listen --seconds 5
```

`doctor` names the device the work will land on and `listen` prints the
real-time factor, so a build running on the CPU is visible immediately rather
than at M3. The two are separate questions and `doctor` answers both — see
[Which GPU build](#which-gpu-build).

### Polishing

The stage that makes Klar not a transcriber. It runs against a model on your own
machine through [Ollama](https://ollama.com), and it is off until you point it at
one — dictation works without it and inserts the transcript as recognised.

```sh
ollama pull llama3.2:3b     # or whatever size this machine can answer with fast
cargo run -p klar-cli -- polish "so I guess we should um push the review to \
  Thursday no Friday" --model llama3.2:3b
```

It prints what went in, what came back, and how long against the 400 ms budget.
`--strength verbatim` sends nothing anywhere; `light`, `balanced` and `heavy` use
the prompts in `crates/klar-core/prompts/polish/v1/`.

The fixture set checks that a model cleans a dictation rather than answering it:

```sh
KLAR_OLLAMA_MODEL=llama3.2:3b cargo test -p klar-core --test polish_fixtures
```

Without that variable it skips, because CI has no model server.

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

### The dictionary

Whisper mangles names it has never seen, and it mangles them the same way every
time — which is what makes it fixable.

```sh
cargo run -p klar-cli -- dict add Anthropic --sounds-like anthropik \
  --sounds-like "and thropic"
cargo run -p klar-cli -- dict try "I work at anthropik on analysis"
#   in    I work at anthropik on analysis
#   out   I work at Anthropic on analysis
```

`dict try` needs no model and no microphone, which is the point: an entry can be
checked against a sentence before speaking into it. `dict list|enable|disable|remove`
and `dict prompt` — what the dictionary contributes to whisper before decoding
— round it out.

`history` and `stats` show what has been dictated on this machine.
`history --clear` deletes the text and keeps the totals; `stats --clear` erases
those separately, because "forget what I said" and "forget that I was here" are
different requests.

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

## Building an installer

```powershell
npm install
npm run tauri build -- --features cuda --config src-tauri/tauri.cuda.conf.json
```

The installer lands in `src-tauri/target/release/bundle/nsis/`. It installs for
the current user only, so it needs no administrator.

`--features cuda` matters: without it the build works, runs on the CPU, and
misses the latency budget by an order of magnitude. The extra `--config` is what
packages the CUDA runtime — see below. Leave both off and you get a working CPU
installer.

### Which GPU build

The backend is chosen at compile time, so one binary cannot cover every machine.
There are three answers and they are not equal:

| Build | Runs on | Needs at build time | Carries at runtime |
|---|---|---|---|
| `--features cuda` | NVIDIA only | CUDA Toolkit | ~250 MB of NVIDIA runtime, bundled |
| `--features vulkan` | AMD, Intel, NVIDIA | Vulkan SDK (`glslc`) | nothing — `vulkan-1.dll` comes with the graphics driver |
| no feature | anything | nothing | nothing |

**A CUDA build on a machine without an NVIDIA card is the trap.** It installs,
it starts, it transcribes, and ggml falls back to the CPU — so it is roughly ten
times slower and nothing about it looks broken. Klar now says so out loud:
`Acceleration::probe` reads ggml's device registry at model load, and the answer
appears in the log, in `klar-cli doctor`, and in Settings → Voice → Processing,
which reads *On this PC — NVIDIA GeForce RTX 4070 (cuda)* when it is right and
explains the mismatch when it is not.

```sh
cargo run -p klar-cli --features vulkan -- doctor      # what did ggml find?
cargo run -p klar-cli --features vulkan -- dictate     # what does it cost?
```

Vulkan is the portable answer and the one to build for anybody who is not on
NVIDIA. It compiles, its device probe is exercised in CI, and it runs: on an
RTX 5070 Ti, **median 296 ms** from key-up to inserted text over four
dictations, worst 317 ms, against a 500 ms criterion — on the same machine where
CUDA measures a 260 ms median. Four is not M3's twenty, so treat it as a reading
rather than a pass, but it is the right order of magnitude, which is what the
decision between one installer and two rests on.

The very first dictation on the first Vulkan run took **17.4 seconds**. Vulkan
compiles its compute pipelines the first time each shader is used, and loading a
model touches none of them, so the cost landed on whoever spoke first. Model
load now runs a throwaway inference to move it — see `WhisperTranscriber::warm`,
whose comment is honest that the 17 seconds has not been seen again and that a
400 ms warm-up cannot have paid for it. NVIDIA's on-disk shader cache is the
likely reason, and nobody has tested against a cleared one.

Two installers built from the same version produce the same filename, so rename
them (`Klar_x.y.z_x64-setup.exe` → `…-cuda-setup.exe`, `…-vulkan-setup.exe`)
before publishing both.

**Bump the version with the change that goes out.** Klar is unsigned and has no
auto-update, so an installer lives on somebody's machine until they replace it
by hand, and the number in its filename is the only way anyone can tell which
Klar they are running. It lives in `Cargo.toml`, `package.json` and
`src-tauri/tauri.conf.json`, which must agree — the last one names the
installer. [`CHANGELOG.md`](CHANGELOG.md) says what each one changed.

### Why the CUDA runtime is bundled

whisper.cpp links `cudart`, `cublas` and `cublasLt` dynamically, so a machine
without the CUDA Toolkit cannot start Klar at all: it fails at load time with a
missing-DLL box, before any Klar code runs and before anything can explain
itself. The toolkit is a multi-gigabyte developer download and nobody should
need it to use a dictation app.

`src-tauri/build.rs` copies those three libraries out of `%CUDA_PATH%\bin` into
`src-tauri/cuda-runtime/`, and `tauri.cuda.conf.json` packages them beside the
executable, which is where Windows looks. NVIDIA permits redistributing them
with an application.

They are in a separate config file because Tauri treats a resource glob that
matches nothing as an error, and a CPU build has nothing to match.

A Vulkan build needs none of this and takes the plain config: the loader,
`vulkan-1.dll`, is installed by the graphics driver on every machine that has a
GPU worth using, and the shaders are compiled into the binary at build time.
That is a quarter of a gigabyte the installer does not carry.

### Updates

Klar checks for a newer version at startup and can install one from Settings →
General → Updates. Two things have to exist for that to work, and only one of
them is in this repository.

**One: a signing keypair.** Every update is signed, and the public half is
compiled into the app; an installer that does not verify is refused before it
runs. This is separate from Windows code signing below — that one tells Windows
who wrote the app, this one stops the update channel itself from being a way in.
Generate it once and never lose the private half:

```powershell
npm run tauri signer generate -- -w $HOME\.klar\updater.key
```

Put the **public** key in `src-tauri/tauri.conf.json` under `plugins.updater.pubkey`,
and set the private half in the environment of whatever builds releases:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $HOME\.klar\updater.key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "..."      # what you chose
npm run tauri build -- --features vulkan --config src-tauri/tauri.release.conf.json
```

That extra config turns on `createUpdaterArtifacts`, which produces a `.sig`
beside the installer. Without it you get an ordinary installer and no update.

**Two: a manifest on the site.** `plugins.updater.endpoints` points at a JSON
file — change the host there to yours. It looks like this, and the `signature`
is the contents of the `.sig` file the build produced:

```json
{
  "version": "0.2.0",
  "notes": "The dictionary, history and statistics.",
  "pub_date": "2026-08-06T12:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<contents of Klar_0.2.0_x64-setup.exe.sig>",
      "url": "https://klar.app/downloads/Klar_0.2.0_x64-setup.exe"
    }
  }
}
```

Klar compares `version` against its own and offers the update when it is newer.
Serve it over HTTPS.

**Publish the Vulkan build to the update channel.** There is one
`windows-x86_64` key and two flavours of installer, and Vulkan is the one that
runs on every card. A CUDA build offered to somebody with an AMD card would
install, work, and be ten times slower.

**An update replaces the previous Klar; it does not sit beside it.** The
installer keys off `identifier` (`app.klar.desktop`) and installs to a directory
named after `productName`, and neither carries the version — so 0.2.0 finds
0.1.0's registry entry, removes it and takes its place. That is also why the
identifier must never change.

### Code signing

Unsigned, Windows SmartScreen shows "Windows protected your PC" on first run.
Everything except the certificate is already configured — see the section above
this one for the digest and timestamp settings, and note that the timestamp is
the part people forget, without which a signature dies with its certificate.

Getting a certificate is the part that costs money, and the landscape changed:
since 2023 the private key must live on a hardware token or in a cloud HSM, so a
`.pfx` file you can copy around is no longer issued.

- **Azure Trusted Signing** is the cheapest way in — roughly the price of a
  coffee per month rather than several hundred a year — and it needs no token in
  the post. It signs through a cloud service, so it fits a build script. It
  requires a verified organisation, or an individual identity with three years
  of history.
- **A traditional OV certificate** from Sectigo, DigiCert or similar runs a few
  hundred a year and arrives on a USB token, which means release builds happen
  on a machine with that token plugged in.

Neither makes SmartScreen quiet immediately: it trusts reputation, which
accumulates over installs. An EV certificate skips that wait and costs more.

With a certificate in the Windows store, set its thumbprint in
`bundle.windows.certificateThumbprint`:

```powershell
Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Format-List Subject, Thumbprint
```

Token- and HSM-based certificates never appear there; those use
`bundle.windows.signCommand` instead, pointing at the vendor's signing tool.

A self-signed certificate is worth ten minutes before buying one. It proves the
whole pipeline signs, timestamps and installs, and it changes nothing about
SmartScreen — which trusts issuers, not signatures.

```powershell
$cert = New-SelfSignedCertificate -Type CodeSigning -Subject "CN=Klar Test" `
  -CertStoreLocation Cert:\CurrentUser\My
$cert.Thumbprint
```

### Still to do before this goes to anyone else

- **macOS.** Hardened runtime, Developer ID, notarization and a DMG, none of
  which can be prepared from a Windows machine, and the platform backend is
  still a stub.

## Licence

MIT.
