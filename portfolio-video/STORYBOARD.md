# Klar — portfolio loop: storyboard and plan

Silent, seamless 22 s loop. 30 fps, 660 frames. The same timeline is used for two
compositions: 16:9 (1920 × 1080) and 1:1 (1080 × 1080). Each is laid out separately,
not cropped from the other.

Status: **waiting for approval.** Nothing is built yet. The decisions at the end
need an answer, or a "go with your recommendations".

---

## 1. What the code says

I checked the brief's design system against the source. It matches, with three
details the video will follow:

| Brief | Source | Note |
|---|---|---|
| Archivo throughout | `tokens.css` `--font-heading/body` | Loaded from `@fontsource/archivo`, the app's own package, so nothing is fetched at render time |
| Zero radius, 2 px rules | `--radius-* : 0`, `--rule: 2px` | |
| Red `#EC3013`, dark `#FF563C` | `--color-accent`, `--color-accent-500` | |
| Ground `#F3F2F2`, ink `#201E1D` | `--color-bg`, `--color-text` | The dark *window* ground is `#2D2B2B` (neutral-900). Ink `#201E1D` is the backdrop behind it |
| Pill 260 × 64 | `overlay.css .pill` | **It widens to 360 px once it has text to show** (Polishing, Done) over `--duration-collapse` 420 ms on `--ease-resolve`. The video keeps that |
| States: recording → polishing → done | `Overlay.tsx STATUS` | Labels: **Listening**, **Polishing**, **Done**. Red only while Listening ("red means live"); the pill goes grey once there is text. The real pipeline also passes through **Thinking** for up to 400 ms. See decision 2 |

Other facts from the code that the video uses:

- While Listening, the pill shows a counter: `0:03 — hold CTRL SPACE`. That is the
  Windows default, and Windows ships first.
- During Polishing the real pill shows the **raw transcript**. At Done it shows the
  **clean text**. The engine sends both as text events.
- The overlay sits bottom-centre, 12 % of the screen height above the bottom edge
  (`overlay.rs`).
- A History row reads `time · app · latency ms · polished`, then the clean text, then
  `heard: <raw>` (`History.tsx`).
- The Voice panel's first row is **Processing**. Its hint reads: *"Speech is
  recognised on this machine and never leaves it."* (`Voice.tsx`)

## 2. What is real and what is built for the video

**Imported unchanged from `../src`.** These files are read only; nothing in the app
changes.

- `src/overlay/Overlay.tsx` and `overlay.css`: the pill. Same markup, CSS, labels and
  waveform maths.
- `src/sections/History.tsx`: the real History list, including its own time
  formatting.
- `src/sections/Voice.tsx` and `src/components/Row.tsx`: the real Voice panel,
  including the Processing row.
- `src/App.css`, `src/styles/tokens.css` and `src/styles/global.css`.

**Recreated from source, because it cannot run outside Tauri.**

- The settings window shell (sidebar and panel header) from `App.tsx`. That
  component loads settings over IPC and checks it is inside the Tauri shell. Its
  markup is about 30 lines, and I copy them with the same class names.
- The IPC layer. At bundle time, `src/lib/settings.ts` and `src/lib/ipc.ts` are
  swapped for local mocks that return the fake data below. The real History and Voice
  components can't tell the difference.

**Built for the video only.**

- **The email compose window.** It is generic and unbranded. It is deliberately
  quieter than Klar: neutral hairline rules and no red, so the 2 px rule and the
  accent stay Klar's.
- **Also video-only:** the ghost line, the captions, the end card and the camera.

**One technical catch.** The app animates with CSS transitions, which run on wall-clock
time. A frame-by-frame renderer would capture them at random points and the video
would flicker. So the video switches transitions off in a video-only stylesheet and
redraws each one from the frame number, using the same token values:

- Width: 420 ms on `--ease-resolve`.
- Colour: 220 ms linear.
- Bar heights: 130 ms linear, fed at the engine's real rate of one level every 50 ms.

## 3. Storyboard (16:9 timings; 1:1 uses the same timings)

Camera moves are slow push-ins and pans on a heavily damped spring with no
overshoot. The UI's own motion uses the app's tokens (`--ease-resolve`, 420 / 220 /
180 ms), slowed slightly where the video needs room to breathe. There are no
whip-pans, zooms-through, blurs or bounces.

| Time | Frames | Beat | Caption |
|---|---|---|---|
| 0.0–1.0 s | 0–29 | **Loop seam.** The end card is held, identical to the last frame. At 0.5 s the wordmark, tagline and URL fade out; the `#F3F2F2` ground stays. | — |
| 1.0–2.0 | 30–59 | **Scene 1, compose.** A plain email window rises 16 px and fades in. *To: team@example.com · Subject: Today's release.* The body is empty, with a blinking caret. A slow push-in starts (1.00 → 1.04). | — |
| 2.0 | 60 | The pill slides up 16 px and fades in, bottom-centre. It reads **LISTENING** in red, with the counter `0:00 — hold CTRL SPACE`. The live waveform shows 12 red bars. | **Just speak.** |
| 2.4–6.2 | 72–186 | A **ghost line** appears word by word above the pill in faint neutral-500, timed to the waveform's syllables: *hey so um the build is green now uh can you can you merge it before the standup*. The counter ticks up to 0:04. | Just speak. |
| 6.4 | 192 | **Scene 2, key release.** The pill reads **POLISHING**. It widens from 260 to 360, the bars flatten into grey blocks, and the raw transcript appears inside it. In the ghost line the fillers (*so, um, uh,* the repeated *can you*) fade first, then the line dissolves. The camera eases toward the pill (→ 1.10). | **Klar cleans it up.** |
| 7.5 | 225 | **Scene 3.** The pill reads **DONE** and its text changes to the clean sentence. | Klar cleans it up. |
| 7.6–8.4 | 228–252 | The clean text lands in the email body, word by word: *Hey, the build is green now. Can you merge it before the standup?* The camera pans up to the body. | Klar cleans it up. |
| 9.6 | 288 | The pill dismisses: it fades and drops 8 px. The finished email holds. | (caption out at 10.4) |
| 10.8–11.2 | 324–335 | A calm 12-frame crossfade with a slight scale. The backdrop changes to ink `#201E1D`. | — |
| 11.2–14.8 | 336–443 | **Scene 4, History (dark).** The real History panel shows the new entry on top: `09:41 AM · Mail · 812 ms · polished`, the clean text, and `heard: hey so um the build…` beneath it. Three older entries sit below. A slow push-in holds on the new entry. | **Every word, kept.** |
| 14.8–15.0 | 444–449 | **Scene 5.** The sidebar selection moves from History to Voice. The app itself has no motion here, so the panel crossfades over 6 frames. | — |
| 15.0–18.4 | 450–551 | **Settings → Voice.** The camera pushes in on the Processing row (`On this PC — NVIDIA GeForce RTX 4060 (cuda)`). A 2 px `#FF563C` underline draws left to right under *"Speech is recognised on this machine and never leaves it."* The other rows dim to about 35 %. | **Private. On-device.** |
| 18.4–19.2 | 552–575 | **Scene 6.** The window recedes (scale 0.98, fade) and the ground crossfades from ink to `#F3F2F2`. | — |
| 19.0–19.6 | 570–587 | The end card rises in, staggered: the **Klar** wordmark (Archivo 800, −3 % tracking), a short red rule, *Speak. Get clean text.*, then `getklar.net` in the label/figure style. | — |
| 19.6–22.0 | 588–659 | Hold. **Frame 659 is identical to frame 0.** | — |

**Captions.** Archivo 800 at the `display-m` size, set bottom-left on the same left
margin as the end card. Each one fades in with a 12 px rise (ease-out, 0.4 s) as its
action starts and fades out over 0.25 s. They are ink on light scenes and `#F8F4F4`
on dark ones.

**End card.** It is set flush left, as the windows are ("rows sit on a single left
edge"). The only accent is one short red rule, taken from the installer sidebar
artwork.

### Layout per format

The stage is a logical canvas drawn at 1.5×, so all UI renders at true proportions
with crisp vector text.

**16:9 (logical 1280 × 720)**

```
┌──────────────────────────────────────────────────────────────┐
│          ┌────────────── New message ──────────────┐         │
│          │ To        team@example.com              │         │
│          │ Subject   Today's release               │         │
│          │                                         │         │
│          │ Hey, the build is green now. Can you…|  │         │
│          │                                  [Send] │         │
│          └─────────────────────────────────────────┘         │
│           hey so um the build is green now uh can you…       │ ← ghost line
│                    ┌──────────────────────┐                  │
│                    │ ▌▌▌▌▌▌  LISTENING     │                  │ ← pill, 12 % up
│ Just speak.        └──────────────────────┘                  │
└──────────────────────────────────────────────────────────────┘
```

History and Voice: the 900 × 620 window sits centred, slightly enlarged, with its
lower part cropped by the push-in. The caption sits on the ink backdrop.

**1:1 (logical 720 × 720), recomposed**

- **Compose.** The window is narrower (about 620 × 380) and higher in the frame. The
  ghost line wraps to two centred lines. The pill keeps its true 12 % position. The
  push-in toward the pill is tighter (→ 1.14) before panning up to the body.
- **History.** It is framed off-centre: sidebar and entry column in frame, the
  window's right edge running off the side. There is a slow pan from the active
  "History" item to the new entry.
- **Voice.** A tight frame on the Processing row, pushed in further than the 16:9
  version (≈ 1.35) so the highlighted sentence reads large.
- **End card.** Flush left, with the wordmark sized to the square.

### Fake demo data (nothing real)

- **Email.** `team@example.com` (a reserved documentation domain); subject
  *Today's release*.
- **History.** App names are generic: Mail, Notepad, Code.
  - `09:41 AM · Mail · 812 ms · polished`: *Hey, the build is green now. Can you merge
    it before the standup?* / heard: *hey so um the build is green now uh can you can
    you merge it before the standup*
  - `09:12 AM · Notepad · 640 ms · polished`: *Move the review to Friday and I'll write
    up the notes after.* / heard: *move the review to thursday no friday and uh i'll
    write up the notes after*
  - `08:57 AM · Code · 590 ms · polished`: *Rename the helper to parseConfig and add a
    test for empty input.* / heard: *rename the helper to parse config and um add a
    test for empty input*
  - `Yesterday 6:05 PM · Mail · 701 ms · polished`: *Thanks, that works for me. Let's
    pick it up tomorrow.* / heard: *thanks that works for me uh let's pick it up
    tomorrow*
- **Voice.** Model `large-v3-turbo-q5_0`, language English, cleanup Balanced, polish
  model `qwen3-4b-instruct-q4`, version `0.3.2 · windows x86_64`.

## 4. Build plan

```
portfolio-video/
├── package.json          # its own deps; nothing added to the app's package.json
├── remotion.config.ts    # webpack: one React copy, the ../src imports, IPC mocks swapped in
├── tsconfig.json
├── src/
│   ├── index.ts, Root.tsx        # two compositions: Klar16x9, Klar1x1 (660 f @ 30 fps)
│   ├── Video.tsx                 # the one timeline; layout chosen by format
│   ├── scenes/                   # Compose, History, Voice, EndCard
│   ├── ui/                       # ComposeWindow, SettingsWindow (shell from App.tsx), Pill (wraps the real Overlay)
│   ├── mocks/                    # settings.ts, ipc.ts: fake data
│   └── motion/                   # tokens as easings, camera springs, speech envelope for the waveform
├── scripts/render.sh             # renders and encodes everything, then checks sizes
└── out/
```

- **Dependencies.** `remotion` and `@remotion/cli` pinned exactly (4.0.535 is current
  today; I'll pin whatever is current at install time). React 19.2 and
  `@fontsource/archivo` 5.2.x match the app's versions.
- **Fonts.** Rendering waits for all five Archivo weights to load, so no frame is
  drawn in a fallback face.
- **The app's lint.** The app's CI runs `eslint .` from the repo root, which will
  lint this folder's TypeScript too. The video code is written to pass the app's
  existing rules, and I'll check that before pushing. The root config is not touched.

**Rendering.**

- Remotion renders a lossless PNG master per format, using the headless Chromium
  already on this machine.
- The system ffmpeg then encodes it twice:
  - **MP4:** H.264 `libx264`, `yuv420p`, `-preset slow -tune animation`,
    `+faststart`, no audio track.
  - **WebM:** VP9 `libvpx-vp9`, two-pass constrained quality, no audio track.
- Both are tagged BT.709, so `#F3F2F2` decodes as `#F3F2F2` in a browser and doesn't
  shift.
- CRF starts at about 18 (H.264) and 30 (VP9). I'll lower it as far as the 5 MB
  limit allows. Flat UI compresses well, so I expect roughly 1.5–3 MB at visually
  lossless settings.

**Checks before handing over:**

- Every file is under 5 MB.
- `ffprobe` shows no audio stream.
- Frame 0 and frame 659 are pixel-identical in the master.
- The ground decodes back to `#F3F2F2`, within ±1.
- I look at the poster and the key stills.

**Process, as you asked.**

1. This document. ← **we are here**
2. On approval: build, render a half-resolution preview, and send you stills of
   5 key frames: listening, polishing, done with text in the email, History, and
   Voice highlighted.
3. On your OK: render the final files to `out/`: `klar-16x9.mp4`, `klar-1x1.mp4`,
   `klar-16x9.webm`, `klar-1x1.webm` and `klar-poster.png` (frame 0 of the 16:9).

## 5. Decisions for you

1. **Loop seam.** *Recommended:* the video opens on the end card for its first second
   and the seam falls inside that still hold, so the loop is invisible. The poster,
   which is the first frame, is the branded card. *Alternative:* open on the empty
   compose window and fade the end card back to it in the last second. The poster then
   shows the product, but the end card gets about 1 s less hold and the loop crosses a
   transition.
2. **The "Thinking" state.** The real app shows it for a few hundred ms between
   Listening and Polishing. *Recommended:* leave it out, as in your storyboard. At
   video pace a 0.3 s label reads as a glitch. *Alternative:* show it for 0.3 s, to be
   strictly true to the code.
3. **How the clean text arrives.** Klar actually pastes in one go, within a 50 ms
   budget, so a slow typewriter would suggest it is slow. *Recommended:* the words land
   left to right over about 0.8 s. That reads as typed and stays honest. Say if you
   want a true per-character typewriter.
4. **Fake data.** Are the app names (Mail / Notepad / Code), the
   GPU string and the History entries above fine?
5. **Committing the renders.** *Recommended:* commit `out/` to this branch, about
   10–15 MB in all. This container is temporary, so that is how the files survive.
   *Alternative:* I send you the files and keep `out/` out of git.
6. **Extra poster.** Should I also add `klar-poster-1x1.png` for the square video's
   `poster` attribute? *Recommended:* yes. It costs nothing.
