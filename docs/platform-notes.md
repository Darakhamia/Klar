# Platform notes

What was tried, what the OS did, and what it cost. Written down as it happens —
the same problems come back at M7 during signing, and nobody remembers by then.

Newest first.

---

## M7 — shipping

### "Which backend was compiled" is not "which device was found"

`Backend::compiled()` answers a build question and I had been treating it as an
answer about the machine. It is not the same question, and they come apart in
the case that matters most: a CUDA build on a computer with an AMD card. The
bundled `cudart64_*.dll` loads, `cudaGetDeviceCount` finds nothing, ggml falls
back to the CPU, and the app installs, starts, transcribes and is roughly ten
times slower — with `asr ready backend=cuda` in the log, which is true and
completely misleading.

Nothing in the pipeline noticed. The module that logs the backend opens with a
comment about how a silent CPU fallback is the failure this project must not
have, and it could not see this one, because it was reporting a `cfg!` rather
than asking anything.

ggml keeps a registry of the devices its compiled backends actually found, and
`whisper-rs-sys` binds it: `ggml_backend_dev_count`, `_get`, `_name`,
`_description`, `_type`, `_memory`. Reading it costs nothing, works before any
model is loaded, and turns the log line into `built for cuda, found no GPU`.
That lives in `klar-core::asr::devices` and surfaces in three places — the log,
`klar-cli doctor`, and Settings → Voice → Processing.

The registry is populated during static initialisation of the linked ggml, so
it can be read before a `WhisperContext` exists. This is deliberate: the answer
decides what to tell somebody whose model has not finished downloading, and
making them wait for a slow first dictation to find out is the situation being
fixed.

Integrated graphics register as `GGML_BACKEND_DEVICE_TYPE_IGPU`, separate from
`GPU`. Counted as acceleration — it is — but worth telling apart when
explaining a disappointing measurement on a laptop.

### Vulkan is the portable backend, and it costs nothing to ship

whisper.cpp's Vulkan backend runs on AMD, Intel and NVIDIA. It needs `glslc`
from the Vulkan SDK at build time, and at runtime it needs nothing at all: the
loader `vulkan-1.dll` is installed by the graphics driver, and the shaders are
compiled into the binary. So a Vulkan installer carries none of the ~250 MB of
NVIDIA runtime a CUDA one does.

The feature was declared in both manifests from M1 and had never been compiled.
It builds — verified on Linux with `libvulkan-dev` and `glslc`, which is enough
to prove the feature wiring and the shader step, and not enough to say anything
about latency. Its speed against CUDA on the same card is unmeasured, and until
somebody runs `klar-cli --features vulkan dictate` on real hardware, "Vulkan
works" means it runs.

---

## M6 — interface

### "Type and copy" cannot be an inject followed by a copy

Injection borrows the clipboard: snapshot, set our text, synthesise Ctrl+V,
restore. The restore has to wait — `SendInput` returns as soon as the events
are queued and the target reads the clipboard whenever it gets to the
keystroke — so it runs on a background thread 150 ms later.

Which means a copy issued straight after `inject` returns is silently
overwritten a moment afterwards. The order was right and the timing was not.
The injector needs a mode that skips the snapshot and leaves our text where it
is, so `inject_and_keep` is a trait method rather than two calls at the call
site. It is the one path allowed to lose the previous clipboard contents,
because replacing them is exactly what the user asked for.

### Rebinding needs a second hook, not a mode on the first

The push-to-talk hook and a capture hook want opposite things: one swallows a
known key forever, the other swallows one unknown key once. Threading a mode
through `HookContext` would have put a branch in the callback that runs inside
the system's input path, where every added test is paid for on every keystroke
on the machine.

So capture is its own `WH_KEYBOARD_LL` hook on its own thread, with its own
thread-local.

The two hooks coexist, and the engine keeps running while a chord is captured.
Windows calls the most recently installed hook first, and the capture hook
swallows every non-modifier key-down it is offered, so the push-to-talk hook is
never handed the key and cannot start a dictation out from under the rebind.
`klar-cli rebind --with-hook` exists to check that this is still true.

I first built it the other way — stop the engine, capture, restart — and it did
not work in the app while the identical platform call worked from the CLI. The
join was the only link in that path that could block, and it was buying nothing
the hook chain was not already giving. Removing it is a simplification, not a
diagnosis: I never proved the join was where it hung.

Modifiers are read with `GetAsyncKeyState` at the instant of the key-down and
sent as a bitmask, not a `Vec`: the callback must not allocate. Modifier
key-downs are passed through rather than captured — the user is still building
the chord, and swallowing Ctrl would stop the next key seeing it held.

### Start with Windows is one registry value

`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. No elevation, nothing
outside the user's own hive, and it shows up in Task Manager's Startup tab
where users go to turn things off. The value is quoted, because Windows splits
an unquoted Run value on spaces and `C:\Program Files\Klar\klar.exe` would be
run as `C:\Program`.

Because the user can remove it from Task Manager while Klar is not running,
`settings_get` reports what the registry says rather than what the JSON file
says. The stored field is what they asked for; the registry is what is true.

### Letting a window log into the same file settled three wrong theories

Rebinding did not work in the app while the identical platform call worked from
`klar-cli`, and I spent three rounds inferring what the settings window saw
from what Rust logged. Two of those inferences were wrong. The one that
mattered — "the main window is not receiving events" — was wrong in a way that
sent the whole investigation sideways, because it was consistent with
everything Rust could see.

`ui_log` gives a window a line in Klar's own log. The first run with it in
produced `ui: first engine event received: ready`, which killed that theory
outright, and `ui: rebind: asked` followed by `ui: rebind: rejected with
Nothing was pressed`, which showed the bridge working in both directions and
narrowed the fault to one place: the capture hook does not see keystrokes while
the app's push-to-talk hook is also installed.

Two halves that fail independently need one log, not two. This should have gone
in at M6 with the first window.

### It was never the hooks. It was the list of keys.

`klar-cli rebind --with-hook` settled it: two hooks coexist exactly as
documented, the capture one wins the key, and one run printed

```
chord captured vk="0xbf" mask=1 seen=2
refused   Klar binds a letter, a digit, Space or F1–F12. That key is none of them.
```

`0xBF` is the `/` key. The hook had seen everything all along; the key set
refused it, and refused `,` `.` `\` `'` with it — which is most of what a person
reaches for when they want a hotkey that is not already taken. Four rounds of
looking at the wrong layer, when a message the user could see was naming the
real problem the whole time.

There is no key set now. Capture never rejects a key: it names the common ones,
and anything else is carried by its virtual key code rather than dropped. The
character variant is resolved through `MapVirtualKeyW(MAPVK_VK_TO_CHAR)` and
back through `VkKeyScanW`, so `/` reads as `/` on the layout it was pressed on.
Letters and digits keep their fixed virtual keys instead, so a Cyrillic layout
does not lose them.

One rule survives, and it is about what the key does rather than which key it
is: a key that types a character needs a modifier, because bound bare the hook
would swallow every one the user typed. A key that types nothing — a function
key, Insert, an arrow — may be bound alone.

### The hook is not handed keystrokes while Klar's own window has focus

The user found it, after I had spent five rounds on threads, hook chains,
capture flags and event delivery: **rebinding worked when the terminal window
had focus and never when Klar's own window did.**

That explains every measurement. The push-to-talk hook is fine — it is handed
keys pressed at other applications, which is the only case dictation cares
about. It is not handed keys pressed at Klar's window. And rebinding is the one
feature where Klar's window necessarily has focus: the user just clicked a
button in it.

So rebinding does not use the hook at all. The settings window reads the chord
from its own `keydown` events, which is what every application's shortcut picker
does and what I should have written in the first place — a window that has focus
is handed its own key events by definition, and no global hook is involved. It
sends the `KeyboardEvent.code` and the modifiers to Rust, and the platform layer
decides what key that is, so nothing about the keyboard is decided in
TypeScript.

The hook still has to stand down for the moment of capture: it would otherwise
swallow the *current* hotkey before the window saw it, and start a dictation
instead. That is `suspend`, one atomic and one branch.

Everything else that came out of those five rounds is gone with it — the capture
hook, the temporary hook, the chord atomics, the event counters, the hook
census. It was all scaffolding around a hook that was never going to receive
these keys.

The lesson worth keeping is not about hooks. Two of my five rounds were spent
confidently reporting causes I had inferred from Rust's logs alone, and one of
those inferences was wrong in a way that took the investigation sideways. The
thing that actually solved it was the user noticing which window had focus.

### Installing a second low-level keyboard hook silenced the first

Superseded by the entry above — the second hook was never the problem — but kept
because the measurement was real and the trap is still there for anything that
installs one.

The measurement, from one run:

```
17:58:44  dictation delivered            ← push-to-talk hook working
17:58:47  capture armed   push_to_talk_seen=59
17:58:57  capture timed out  seen=0  push_to_talk_seen=59
```

Fifty-nine key events before the capture, none during it. The push-to-talk hook
had just carried a whole dictation; three seconds later, with a second
`WH_KEYBOARD_LL` hook installed, it was handed nothing for ten seconds while the
user pressed keys. Both hooks silent, in a process where the keyboard had
demonstrably been reaching one of them.

I have no explanation for that. `SetWindowsHookExW` succeeds, the capture
thread pumps, and neither callback is called. It does not reproduce in
`klar-cli`, where the same two hooks coexist and capture works.

So capture does not install a second hook when one is already there. `HOOKS`
counts the push-to-talk hooks this process has, and `capture_binding` installs
its own only when that count is zero — `klar-cli`, or the app before its engine
has loaded. Otherwise it raises `CAPTURING` and the push-to-talk hook records
the chord into three atomics and swallows the key.

Written down because it is a workaround, not a fix. If a second hook is ever
needed for something else, this is the trap it will fall into, and the counters
are still in place to catch it: every hook counts what it is handed, `SEEN`
during a capture and `HOOK_SEEN` always, and both are logged when a capture arms
and when one times out.

### `listen` is ACL-gated per window, and a refusal is silent

The overlay and onboarding both subscribed to `klar://event` and both received
nothing. `capabilities/default.json` said `"windows": ["main"]`, and `listen` is
a core command the capability system gates — a webview not named in any
capability gets its `listen` rejected. App-defined commands from
`invoke_handler` are *not* gated, which is what made this so confusing to look
at: `invoke("models")` worked in the same window that could not subscribe to a
single event.

It went unnoticed because every subscription was written as

```ts
const pending = listen(...);
return () => { void pending.then((unlisten) => unlisten()); };
```

`void` on the cleanup path throws away the rejection. The overlay had been
appearing and rendering its idle frame for a whole milestone — Rust shows and
hides that window, so it looked alive.

Both halves are fixed: the capability names all three windows, and every
subscription goes through `subscribe()` in `src/lib/ipc.ts`, which logs a
refusal and hands it to the caller. Any new window needs a line in
`capabilities/default.json` or it is deaf in exactly the same way.

### Windows has no microphone permission worth asking about

`permission_state(Microphone)` returns `Unknown` on Windows and means it. The
Settings › Privacy › Microphone toggle gates *packaged* apps; a plain desktop
process is not blocked by it, so there is no API that answers "may I record?"
with anything useful.

So onboarding's first step does not ask. It opens the device. If the stream
starts, the answer is yes; if it fails, the error is the real one and the button
next to it opens `ms-settings:privacy-microphone`. The level bars then answer the
question the user actually has, which is not "is permission granted" but "can it
hear me".

macOS will need the real `AVCaptureDevice` check here. The screen is already
shaped for it — the step list comes from what `permission_states` reports, not
from a platform constant, so the accessibility step appears on macOS and is
absent rather than skipped on Windows.

### Onboarding's test field is a real text box

The last step could have shown the transcript from the event stream and called
it a demonstration. It types into a `<textarea>` instead, through the same
clipboard paste every other app gets. That exercises `SendInput`, the clipboard
save/restore, and the elevation check on the way to the user's first sentence —
if injection is broken on their machine, they find out during setup rather than
the first time they try to use the app.

It also means the one place in Klar where `user-select: text` and a caret cursor
are correct is this box. Everything else is an application window.

### The engine starts before the models exist

On a fresh install the engine thread starts, finds no model, and stops with a
message. That is not an error state to recover from, it is the normal first-run
sequence — so onboarding calls `engine_restart` once the download lands, rather
than the engine polling for files that only appear if the user does something.

The same command covers the case where the models were already there: the
`Ready` event fires while the engine loads, which on an existing install happens
before the onboarding window has finished subscribing. Restarting once when the
step opens makes the event arrive after somebody is listening.

### The Tauri code can be type-checked on Linux, and should be

`cargo check --target x86_64-pc-windows-msvc` stopped covering `src-tauri` at
M1, because it depends on `klar-core` and so on whisper-rs-sys, which needs a
real MSVC toolchain. That left the whole Tauri layer unverifiable here.

Installing `libwebkit2gtk-4.1-dev` and `libgtk-3-dev` gets `cargo check -p klar`
compiling on Linux. Linux is still not a target and nothing is run there — but
it catches every misuse of the Tauri API, which is what the check is for.

### Settings are a file, not a database

A dozen fields read once at startup, in JSON in the config directory, written
through a temporary file so an interrupted save cannot truncate it. A settings
file that will not parse is replaced with the defaults rather than being fatal —
the app has to start.

The database M5 builds is for dictations, dictionary terms and statistics:
things there are thousands of. Settings are not that.

### Changing a setting restarts the engine

Most of them decide which model is loaded or which key is hooked, so the engine
is replaced rather than reconfigured. It takes about a second, which is why
there is no Save button — each change applies as it is made, and a setting that
has not taken effect is a lie.

### Three sections with nothing to show

Dictionary, History and Stats all read from the database that does not exist
yet. They say what they will hold and what they are waiting for. Filling them
with invented rows would look finished and be false, and the difference would
only be discovered by someone trying to use them.

### The overlay is a second page, not a second component

It is shown and hidden constantly and must not carry the settings window's code
around with it, so `overlay.html` is its own Vite entry point with its own
bundle. 3 kB against the main window's 193.

### Click-through, and what that costs the error state

The overlay window sets `set_ignore_cursor_events(true)` once and leaves it on.
A window that swallows clicks over someone's editor is worse than no overlay at
all.

The design says the error state waits until dismissed. It cannot be dismissed by
clicking a window that ignores the cursor, so that needs either a timeout or
turning click-through off for that one state. Left open until the error state
has a real user path.

---

## M3 — streaming

### Measured

Key-up to inserted text, `dictate` on the RTX 5070 Ti, mixed Russian dictations
of 2-22 seconds:

| | |
|---|---|
| Median | 260 ms |
| Worst | 537 ms |
| Criterion | median under 500 ms |

A dictation long enough to commit shows the point of the whole exercise: 21.8 s
streamed during speech, 1.1 s of tail, 163 ms from key-up. The same length
without streaming would have been a single pass over the lot.

Note the worst case is a dense ten-second utterance in one pass — more words
means more decoder steps, and that is what the tail time tracks, not the audio
length.

### A diagnosis that did not hold up

Repetition loops were attributed to the `initial_prompt` feedback, on the
strength of four dictations where the loops coincided with commits. A later run
on the same binary produced two commits and clean text, and another produced a
repeat with no commits at all. The likelier cause was the speaker saying the
same line five times over, which whisper is known to turn into a decoder loop
on its own.

The prompt feedback stayed removed regardless — it was speculative, it carries
a known repetition risk, and `min_commit` already gives committed pieces enough
context. But the reasoning in the commit that removed it claimed more than the
evidence supported.

### Silero without a second inference runtime

CLAUDE.md called for the `voice_activity_detector` crate. It pulls `ort`, which
is ONNX Runtime: a build-time binary download, another ~15 MB library in the
installer, and one more thing to sign at M7 — all to run a model of under a
megabyte.

whisper.cpp 1.8 ships Silero itself, and whisper-rs 0.16 exposes it — no second
runtime, and the model is 885 KB. The stack line in CLAUDE.md now says so.

### The VAD does not run on the GPU, and asking kills the process

It shares ggml with the ASR, so running the VAD on CUDA looked free. It is not.
`whisper_vad_init_with_params` with `use_gpu` puts the weights in a CUDA buffer
and then its backend init reports `no GPU found`, leaving tensors somewhere the
compute backend cannot reach:

```
whisper_vad_init_with_params:   CUDA0 total size = 0.88 MB
whisper_backend_init_gpu: no GPU found
ggml-backend.cpp:807: pre-allocated tensor (leaf_0) in a buffer (CUDA0)
                      that cannot run the operation (NONE)
```

ggml calls `GGML_ABORT`, so the process dies — there is no error to catch and
nothing to fall back from. `Vad::load` therefore passes `use_gpu(false)`
unconditionally. At under a megabyte and a few percent of real time this costs
nothing worth chasing, but it is exactly the kind of assumption that has to be
run before it is believed.

### The obvious VAD loop is quadratic

`segments_from_samples` re-runs the model over everything it is given: 231 ms
for an 11-second buffer on CPU. Calling that on each 20 ms capture callback —
which is what the natural implementation does — costs more every second and is
unusable well before a dictation ends.

`StreamingVad` analyses each second of audio exactly once and keeps the
per-frame probabilities Silero produces (one per 512 samples, so 32 ms each).
Segmentation afterwards is arithmetic over that track, which also means the
rules deciding where a phrase ends are plain Rust and directly testable.

`detect_speech` resets Silero's recurrent state on every call, so each block is
analysed with a second of already-seen audio in front of it as warm-up. That
doubles the model work and removes the accuracy cliff at block boundaries.
Measured against the one-shot detector on the same speech: same four segments,
boundaries within 40-160 ms, about 5% of real time on CPU.

### The quadratic cost came back through the side door

`advance` was careful to analyse each second exactly once. Then every partial
called `trim`, which is the one-shot detector, which re-runs the model over the
whole pending buffer — putting back precisely the cost that had just been
designed out. Visible in the log as `whisper_vad_segments_from_samples:
detecting speech timestamps in 37504 samples` on every refresh.

`StreamingVad::speech` cuts the speech out using the probabilities already
stored, with no model run at all. The one-shot detector survives in exactly one
place: the final tail, which includes the last second `advance` has not analysed
yet, and which runs once per dictation over a couple of seconds.

Worth remembering that a careful optimisation is only as good as everything else
on the same path.

### whisper.cpp narrates every VAD call

At info level, once per push. It buries every other line in the log the moment
streaming starts. The default filter demotes `whisper_rs::whisper_logging_hook`
to warn while leaving `ggml_logging_hook` alone, because `ggml_cuda_init: found
1 CUDA devices` is the only runtime proof the GPU was picked up.

The hooks also have to be installed before *any* whisper.cpp context exists —
including the VAD's. A context created first writes straight to stderr, where no
filter can reach it, which is why both `WhisperTranscriber::load` and
`Vad::load` install them.

### Feeding the transcript back as a prompt sends whisper into loops

To give a committed fragment the context it loses by being cut out of its
sentence, the recognised text was passed to the next pass as whisper's
`initial_prompt`. The decoder started repeating:

```
Разбрызгиваю переспокомнатие, чтобы почувствовать себя живым
Разбрызгиваю переспокомнатие, чтобы почувствовать себя живым
Разбрызгива переспокомнатие
```

and, in another run, five consecutive copies of the same three words with one
of them corrupted. Prompt conditioning causing whisper to repeat itself is a
known pathology, and the loops appeared in exactly the dictations that had
commits — never in the ones that finished in a single pass.

Removed. The context it restored was not worth a failure mode that mangles the
output, and `min_commit` already keeps committed pieces long enough to carry
their own context.

### Splicing out internal pauses buys nothing and costs quality

`trim` concatenated the speech segments, dropping the silence between them.
That saves no time whatsoever: whisper's encoder runs over a fixed 30-second
window regardless of how much audio is in it. What it does do is butt phrases
against one another, producing audio no speaker ever made.

It now cuts the ends only — leading and trailing silence, which is where
whisper invents text to fill the gap — and leaves the middle alone.

### Streaming costs accuracy, and the first settings cost too much

Committing every time the VAD saw 500 ms of quiet produced dictations cut
mid-sentence, with each fragment recognised on its own:

```
Мы пойдем в парк кушать. яблоки.
ничего. не показывать
от этого прогулка          (спoken: прогона)
```

Two things going wrong. A 400-500 ms gap is not the end of a sentence, it is
the gap before the next word. And a fragment of a second or two gives whisper
far less to work with than the same words inside a whole phrase, so recognition
genuinely degrades — the last line there is not a cut, it is a misheard word.

What the eager commits bought was measurable: about 180 ms on a ten-second
dictation, tail versus whole. Against a 500 ms budget that a single pass
already clears in 320 ms. Paying accuracy for that was the wrong trade.

`min_commit` is now 10 s, so an ordinary dictation is transcribed in one piece
and streaming changes nothing about it. `commit_silence` is 700 ms and the
VAD's `min_silence` 600 ms, so a commit means a sentence actually ended.
Streaming then does what it is for — keeping a long dictation from arriving all
at once at the end — without touching the common case.

Committed text is also carried forward as whisper's `initial_prompt`, which
restores most of what a fragment loses by being cut out of its sentence.

### Committing at pauses, not at window edges

Whisper is much better on a complete phrase than on an arbitrary slice, so the
commit boundary is a pause the VAD found. That also avoids stitching
overlapping windows token by token, which is where streaming ASR usually goes
wrong. A speaker who never pauses is committed anyway at 15 s, before whisper's
30-second encoder window starts to matter.

### Resampling cannot be done per callback

The resampler is constructed per call, so converting each 20 ms callback
separately leaves a discontinuity at every boundary; converting the whole
buffer again each time is quadratic. `BlockConverter` holds the leftover raw
samples and converts once 250 ms have arrived, always on whole frames so the
channels stay aligned.

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
