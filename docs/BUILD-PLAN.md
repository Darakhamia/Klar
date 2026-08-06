# Klar — build plan

Milestones are ordered by risk, not by how satisfying they are to demo. The two things most likely to kill this project are the whisper.cpp GPU build and hold-to-talk plus text injection across two operating systems. Both are done before any UI exists.

Do not start a milestone until the previous one's acceptance criteria pass on **both** macOS and Windows.

---

## M0 — Scaffold

Cargo workspace with the four crates. Tauri v2 app that opens an empty window. Vite + React + TypeScript frontend. `cargo clippy` and `cargo fmt` clean. CI running build and clippy on both platforms.

**Done when:** `cargo run -p klar-cli` prints a version string and the Tauri app launches on both platforms.

---

## M1 — Speech to text, headless

The riskiest dependency, isolated. No hotkey, no UI, no injection.

`klar-cli record --seconds 5` captures from the default microphone via `cpal`, resamples to 16 kHz mono, runs `whisper-rs`, prints the transcript and the elapsed time.

Get GPU acceleration working now: Metal plus the CoreML encoder on macOS, CUDA on Windows. Log which backend was selected at startup — silently falling back to CPU is a bug that will otherwise hide until the end.

Add model download with resumable progress and SHA verification into the app data dir.

**Done when:** a 5-second Russian sentence and a 5-second English sentence both transcribe correctly, GPU backend confirmed in the logs, and transcription of a 10-second clip finishes in under 1 second on both machines.

---

## M2 — The magic moment

Hold a hotkey anywhere, speak, release, text appears in the focused app. Still no UI beyond a tray icon.

Implement the `Hotkey` and `TextInjector` traits in `klar-platform` with real macOS and Windows implementations. Clipboard-based injection with save and restore. Handle the macOS Accessibility permission: detect whether it is granted, and open the right system settings pane if not.

**Done when:** dictation works into TextEdit, Notepad, a browser text field, VS Code, and Slack on both platforms, and the user's clipboard is intact afterwards — including when injection fails.

---

## M3 — Latency

Move from record-then-transcribe to streaming. Sliding window ASR during speech, VAD to trim silence and to detect the end of an utterance, finalization on key release that only processes the tail.

Instrument every stage and log the timings. Measure against the budget in CLAUDE.md.

**Done when:** median time from key release to inserted text is under 500 ms for a 10-second utterance, measured over at least 20 real dictations per platform.

---

## M4 — Polish layer

The `TextPolisher` trait with three implementations: `Noop`, `Ollama` (local HTTP), `Groq` (cloud). Prompt engineering lives in versioned files, not string literals.

The polish prompt must: remove fillers, fix punctuation and capitalization, resolve spoken self-corrections, format spoken lists, and preserve the speaker's voice and word choice. It must never answer the content, summarize it, translate it, or add anything the speaker did not say. That last constraint is the one that will break — build a small fixture set of real dictations with expected outputs and run them as a test.

A cleanup strength setting maps to different prompts: verbatim, light, standard, heavy.

Verbatim is not a gentle prompt — it is off. The polisher is never constructed and never called, so the transcript goes straight to injection and nothing is sent to a language model, local or otherwise. It must stay possible to run Klar this way indefinitely, and switching to it must take effect on the very next dictation.

**Done when:** the fixture set passes, and polish adds under 400 ms on the local path.

Built. `Noop` and `Ollama` are wired up behind `TextPolisher`; `Groq` is not, and
waits until there is a reason to send anything off the machine. The prompts are
files under `crates/klar-core/prompts/polish/v1/`. Two things were learned
building it and are worth keeping in mind for the cloud path:

- The failure that matters is a model answering the dictation instead of tidying
  it, and no prompt prevents it reliably. `polish::guard` compares lengths and
  throws the result away when it is out of range — crude, and it catches the
  case, because an answer misses by a mile rather than by a word.
- Every failure falls back to the transcript. A model that is down, slow, or
  answering must not cost the user the words they just said.

---

## M5 — Storage

SQLite schema and migrations. Three tables: `dictations` (text, raw text, duration, target app, timestamp), `dictionary_entries` (term, replacements, enabled), `daily_stats`.

Dictionary applies in two places: user terms are injected into the whisper initial prompt to bias recognition, and a substitution pass runs after transcription for anything that still comes out wrong.

**Done when:** a dictionary entry for a name whisper reliably mangles produces the correct spelling end to end.

Built. Three tables, and `daily_stats` is a real one rather than a query over
`dictations`, so clearing the history does not also erase how much somebody has
used the app — the text is the private part, the totals are not. `clear_history`
VACUUMs, because otherwise "cleared" would mean "not shown".

The dictionary applies in both places the plan asks for, and the substitution is
the half worth being careful about: whole words only, longest phrase first,
case-insensitive, and Unicode-aware, because a naive replace turns *analysis*
into *Anasis* for anyone who taught it a name like Ana. Both failures are tests.

It runs before the polish stage, not after. A language model handed a mangled
name tidies it confidently into a different mangled name, and the prompts
already forbid inventing words the speaker did not say.

The dictionary is re-read on every dictation rather than held from startup, so a
word taught in the settings window works on the very next sentence — which is
the only moment anybody will think to test it.

**Not yet verified against the criterion**, which is about a name whisper
mangles *reliably*. That needs somebody's real name and a microphone.

---

## M6 — Interface

Now build the frontend, following the design output. Onboarding with the two permission steps, hotkey selection, model download, and a live test field. The overlay window — borderless, always on top, click-through when idle, rendering all five states from the core state machine. Tray menu. Main window with General, Voice, Dictionary, History, Stats.

The overlay is the piece that carries the product. Give it the time it needs.

**Done when:** a fresh install can be taken from first launch to a successful dictation without touching a terminal or a config file.

---

## M7 — Shipping

macOS: hardened runtime, Developer ID signing, notarization, DMG. Windows: code signing, MSI or NSIS installer. Auto-update via `tauri-plugin-updater`. Crash-free logging with a rotating file and an easy way for a user to export it.

**Done when:** a build installs and runs on a machine that has never had a developer toolchain on it.

**Windows passed.** The Vulkan installer has been installed and used by three
people on three machines — a desktop RTX 5070 Ti, a laptop RTX 3060, and an AMD
Radeon RX 7900 XT — two of which had never had a developer toolchain on them.
That is the criterion, on the backend that can actually be handed to somebody.
Unsigned, so SmartScreen asks first; auto-update and all of macOS remain unbuilt,
and the notes below still describe what it took to get here.

What that criterion was actually failing on was not signing:
the installer would not *start* on a machine without the CUDA Toolkit, because
whisper.cpp links the CUDA runtime dynamically and Windows gives up at load time
with a missing-DLL box, before any of our code can explain itself. `build.rs`
now collects those three libraries and the bundle carries them beside the
executable, so the criterion is reachable.

"A machine that has never had a developer toolchain on it" turned out to hide a
second assumption: that the machine has an NVIDIA card. A CUDA build on a
computer with an AMD one clears the criterion as written — it installs and it
runs — while running every transcription on the CPU at roughly ten times the
budget. Two things came out of that:

- **The mismatch is now visible.** `klar-core::asr::devices` reads ggml's device
  registry at model load and names what will actually do the work, in the log,
  in `klar-cli doctor`, and in Settings → Voice → Processing. M1's "log which
  backend was selected" was answering a `cfg!`, not the machine.
- **There is a portable build.** `--features vulkan` covers AMD, Intel and
  NVIDIA, needs no redistributable runtime, and measures a 296 ms median from
  key-up to inserted text on an RTX 5070 Ti against M3's 500 ms — beside a
  260 ms median for CUDA on the same machine. Four dictations rather than M3's
  twenty, so it is a reading and not a pass; deciding whether one Vulkan
  installer replaces two needs the full set, and on an AMD card, which nobody
  here has.

Signing is configured and unsigned. The digest and the RFC 3161 timestamp URL
are set — the timestamp being the part people forget, without which a signature
dies with its certificate — and only the certificate is missing, because one has
to be bought and issued to a named person. `scripts\sign.ps1` connects it in one
command, and does the self-signed rehearsal worth running first.
[`signing.md`](signing.md) is the decision: what each option costs, and the
honest answer to what any of them does to SmartScreen, which is not what people
expect they are buying.

The installer has its artwork — `src-tauri/installer`, from the design session
briefed in `design/installer-brief.md`. The sidebar says where speech goes
rather than what the app does, on the reasoning that it is read immediately
after Windows has said it does not know who wrote this.

Log export is done and it is not a nicety: Klar has no crash reporter and sends
nothing anywhere, so a problem on somebody's machine reaches nobody unless they
can find the log in one click. Settings → Diagnostics → Show log file.

Not done: auto-update, which needs a release channel to update from and a
keypair, and building it before either exists is scaffolding; and all of macOS,
whose platform backend is still a stub.

---

## M8 — The dictionary that fills itself

Today the dictionary only holds what somebody sat down and typed into it, which
means it holds almost nothing: the words Klar gets wrong are discovered one at a
time, mid-sentence, in the middle of doing something else, and by the time
there is a spare minute they have been forgotten. The goal of this milestone is
that using Klar for a month makes it better at *your* words without anybody
maintaining a list.

### What cannot be done, first

**Klar cannot see you fix a word.** Text is injected into another application
and the story ends there. Watching what happens afterwards means reading other
applications' windows or their keystrokes, and the privacy rules forbid it in
plain terms — nothing about what is dictated leaves the pipeline, and a
background process that reads whatever the user types next is exactly the thing
this app must never be. So the literal request — notice when a word is corrected
by hand — is off the table, and any design that quietly needs it is wrong.

**Whisper cannot be trained here.** Fine-tuning a 574 MB model on a laptop, per
user, is not a feature; it is a research project with a GPU bill. Nothing in
this milestone changes a model's weights. What improves is which words the model
is *told to expect* before it starts, and that turns out to be enough.

### The three signals that are actually available

All three read the SQLite file Klar already writes. None adds a capture path,
and none sends anything anywhere.

**1. What polish already rewrites.** Every dictation stores `raw` — whisper's
transcript — and `text` — what was finally inserted. When the same token in
`raw` becomes the same different token in `text` across many dictations, the
polish model has been quietly making the same correction over and over. That is
a dictionary entry, already written, that nobody has been shown. It costs one
query and no new plumbing, and it is the strongest of the three because the
correction was made by something that had the whole sentence for context.

**2. Saying it again.** A dictation followed within a few seconds by a second
one that is textually close is somebody repeating themselves because the first
attempt came out wrong. The pair of transcripts brackets the word that failed.
Timestamps are already stored; the diff is the candidate.

**3. Correcting a row in History.** Make history rows editable. This is the only
unambiguous signal of the three — the user is deliberately saying "it should
have been this" — and the one nobody will use, because visiting a history pane
to fix a word that has already been sent is not a thing people do. Build it
anyway: it is cheap, and it is the ground truth the other two are guessing at.
Without it there is no way to tell whether the guesses are any good.

### Ranking the prompt by what gets used

The dictionary already biases whisper through `initial_prompt`, and that prompt
has a hard budget — `PROMPT_LIMIT`, 800 characters. A dictionary larger than the
budget is silently truncated today, which means the fiftieth word taught does
nothing at all.

Record which entries actually fire, and spend the budget on the terms this
person says, most-recent and most-frequent first. This is the part that makes
the claim "it gets better the longer you use it" literally true rather than
aspirational, and it is a sort, not a model.

### Rules this milestone does not get to break

- **A suggestion is a proposal, never an edit.** The dictionary changes what the
  user's text says. Software that rewrites somebody's words on a statistical
  hunch, without asking, is the failure mode this whole product is one step
  away from. Suggestions appear in the Dictionary pane with the evidence — the
  dictations they came from, and how many — and do nothing until accepted.
- **Rejecting a suggestion is permanent.** Proposing the same word twice is
  worse than never proposing it.
- **Clearing history clears the suggestions.** Otherwise "clear history" leaves
  behind a list of words derived from the text it claimed to delete, which is
  the same information wearing a different hat.
- **The mining runs on stored rows, not in the dictation path.** M3's budget is
  not to be spent on this. Off the hot path entirely — on a timer, or when the
  Dictionary pane opens.
- **It must be switchable off**, and off means no candidate table at all.

**Done when:** after twenty dictations containing a name whisper reliably
mangles, Klar proposes the correct spelling on its own; accepting the proposal
fixes the next dictation; nothing is ever applied that was not accepted; and
clearing the history leaves no trace of the words it was derived from. Every one
of those is testable from `klar-cli` without a microphone, against a seeded
database.

---

## Working agreement

- Prove risk before building comfort. If something in a milestone looks like it might not work at all, spike it in `klar-cli` first.
- Every milestone ends with a manual test on both operating systems. Not one, both.
- When a platform API fights back, write down what you tried in `docs/platform-notes.md`. The same problem will come back at M7 during signing.
- If a milestone's acceptance criteria cannot be met, stop and say so rather than reducing the criteria.
