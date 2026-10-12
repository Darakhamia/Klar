# Klar — portfolio loop: storyboard and plan

Silent, seamless 22 s loop. 30 fps, 660 frames. The same timeline is used for two
compositions: 16:9 (1920 × 1080) and 1:1 (1080 × 1080). Each is laid out separately,
not cropped from the other.

Status: **approved and built.** Section 5 records the answers. Where the build
departed from this plan after seeing it rendered, the table says so and section 6
lists why.

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
| 6.4 | 192 | **Scene 2, key release.** The pill reads **POLISHING**. It widens from 260 to 360, the bars flatten into grey blocks, and the raw transcript appears inside it. In the ghost line the fillers (*so, um, uh,* the repeated *can you*) fade first, then the line dissolves. The camera closes in on the pill. | **Klar cleans it up.** |
| 7.5 | 225 | **Scene 3.** The pill reads **DONE** and its text changes to the clean sentence. | Klar cleans it up. |
| 7.6–8.4 | 228–252 | The clean text lands in the email body, word by word: *Hey, the build is green now. Can you merge it before the standup?* The camera pulls back to the whole group: the email with its new sentence above the pill reading Done. | Klar cleans it up. |
| 9.6 | 288 | The pill dismisses: it fades and drops 8 px. The finished email holds. | (caption out at 10.4) |
| 11.0 | 330 | **A straight cut** to the dark History window, which settles from 102 % over 0.8 s. *(Planned as a crossfade; see section 6.)* | — |
| 11.0–14.9 | 330–446 | **Scene 4, History (dark).** The real History panel shows the new entry on top: `09:41 AM · Mail · 812 ms · polished`, the clean text, and `heard: hey so um the build…` beneath it. Three older entries sit below. The camera pushes in on the list column until the new entry and its `heard:` line fill the frame; the older entries dim behind the caption. It pulls back out before the sidebar click. | **Every word, kept.** |
| 14.9 | 447 | **Scene 5.** The sidebar selection moves from History to Voice **in one frame**, as a click does in the app. *(Planned as a 6-frame crossfade; see section 6.)* | — |
| 14.9–18.4 | 447–551 | **Settings → Voice.** The camera pushes in until the Processing row fills the frame edge to edge, `On this PC — NVIDIA GeForce RTX 4060 (cuda)` included. The sentence *"Speech is recognised on this machine and never leaves it."* is marked left to right in `#FF563C`. The other rows dim to 14 %. | **Private. On-device.** |
| 18.4–18.9 | 552–565 | **Scene 6.** The window fades out on ink. | — |
| 18.9 | 566 | A straight cut to the `#F3F2F2` ground. | — |
| 18.9–19.6 | 568–589 | The end card rises in, staggered: the **Klar** wordmark (Archivo 800, −3 % tracking), a short red rule, *Speak. Get clean text.*, then `getklar.net` in the figure style. | — |
| 19.6–22.0 | 590–659 | Hold. **Frame 659 is identical to frame 0**, and the render script checks it. | — |

**Poster.** `klar-poster.png` and `klar-poster-1x1.png` are frame 270 (9.0 s), not
frame 0. That is the Done moment: the clean sentence is in the email and the pill
still reads Done.

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

1. This document.
2. On approval: build, render a half-resolution preview, and send you stills of
   5 key frames: listening, polishing, done with text in the email, History, and
   Voice highlighted.
3. Render the final files to `out/`: `klar-16x9.mp4`, `klar-1x1.mp4`,
   `klar-16x9.webm`, `klar-1x1.webm`, `klar-poster.png` and `klar-poster-1x1.png`.

## 5. Decisions (answered)

1. **Loop seam.** The video opens on the end card for one second, so the seam
   falls inside its still hold. The poster is the Done moment (frame 270), not the
   first frame.
2. **Thinking state.** Left out.
3. **Clean text.** It lands word by word, left to right, over 0.8 s.
4. **Fake data.** As listed above. Nothing else real is named.
5. **Committing the videos.** The rendered files live on a separate branch,
   `portfolio-video`, and never in main's history. On every other branch `out/`
   is ignored, and there is no PR from that branch to main.
6. **Square poster.** Yes: `klar-poster-1x1.png`, the same Done moment recomposed
   for the square.

## 6. Changes made after seeing it rendered

- **Cuts instead of crossfades between light and dark.** A crossfade from the
  light email scene to the dark History window, and back to the light end card,
  passes through a flat mid-grey for a third of a second. A cut is cleaner, and it
  matches "cut to History" in the brief.
- **Instant switch from History to Voice.** A 6-frame crossfade put two lists on
  top of each other. The app has no motion on a sidebar click, so the video
  doesn't either.
- **Captions never sit on the interface.**
  - In 16:9 the dark window sits to the right of the caption, or the camera keeps
    the sidebar's rule to the right of where captions end.
  - In the square, the window stays above a band of ink at the bottom, where the
    caption sits.
- **"Clear history" is out of frame** at the end of the History push-in, rather
  than cut in half by the edge of the frame.
- **One thing about the app.** The History list renders 40 px in from its heading.
  That is the browser's default `padding-left` on the `<ul class="hist">`, which
  the app's CSS doesn't reset. The video shows it as the app draws it.

### Second pass: framing for an 800 px card

The video plays in a card about 800 px wide, where the first cut's UI was too
small to read: the email window filled about a third of the 16:9 frame. Every
scene is now framed on boxes measured from the rendered components
(`src/format.ts`).

- **Fill.** A subject fills roughly 70–80 % of the frame. The key moments get
  push-ins: the ghost line and pill while listening, the pill while polishing,
  the finished email, the `heard:` line, and the on-device sentence.
- **Crops are either none or obvious.** A subject is whole with room around it,
  or framed tight enough that the crop is plainly a close-up. In the pill
  close-ups the email's foot shows across the top of the frame, its sides and
  Send button well inside. The History and Voice close-ups run edge to edge
  across the panel's content, with the sidebar rule and the window border just
  out of frame.
- **One group.** The email window, the ghost line and the pill are one centred
  stack.
- **Captions never sit on live text.** Whatever is under a caption is either
  empty or already dimmed. The sidebar dims before a push sweeps its rule past
  the caption.
- **The listening line stays on one line.** In Archivo, `0:02 — hold CTRL SPACE`
  needs 135 px and the 260 px pill gives it 128, so it broke before SPACE. The
  shipped overlay fits it because it never loads Archivo: `src/overlay/main.tsx`
  imports no font, so the pill falls back to the narrower system face. The video
  keeps Archivo and takes 0.04 em of tracking off that line.

## 7. Output

| File | Size |
|---|---|
| `klar-16x9.mp4`: H.264 High, CRF 16 | 4.26 MB |
| `klar-16x9.webm`: VP9, two-pass, CRF 20 | 3.95 MB |
| `klar-1x1.mp4` | 2.53 MB |
| `klar-1x1.webm` | 2.38 MB |
| `klar-poster.png`, `klar-poster-1x1.png` | frame 270 |

- **Format:** all files are 30 fps, 22.000 s and 660 frames, `yuv420p` tagged
  BT.709, with no audio track.
- **Colour check:** the ground decodes back to `#F2F2F2`, within one step of
  `#F3F2F2`.
- **Where to get them:** the `portfolio-video` branch. `README.md` explains how to
  re-render them.

## 8. The portrait card

A 3:4 card for the portfolio's filmstrip carousel. It shows the same moment as
the poster, recomposed for portrait rather than cropped from 16:9.

**Composition.**

- A tall email compose window fills the card on the `#F3F2F2` ground.
- The real overlay floats over the window just under the sentence it typed, as
  the overlay floats over whatever application is in front.
- There are no captions, no wordmark and no end card.

**The clip.** An unfocused card is clipped to its top half at
`object-position: 50% 26%`, which on a 1200 × 1600 card is the band from y 208
to y 1008. Everything that matters sits inside it: the To and Subject rows,
the sentence (set at 22 px) and the pill, which spans about 80 % of the card's
width. The full card adds the window's title bar, its empty body and the Send
footer.

**The loop** (8 s, 240 frames) runs the same scene at a quicker pace:

1. The pill comes in.
2. Speech runs from 0.6 s to 4.0 s, with the ghost line building word by word.
3. Polishing.
4. Done, and the sentence lands.
5. A 1.2 s hold.
6. The pill leaves and the sentence clears, so the loop ends on the same empty
   email it starts with.

The caret holds steady in the loop, so the first and last frames are the same
picture.

| File | Size |
|---|---|
| `card/klar-card.png`: 1200 × 1600 | 174 KB |
| `card/klar-card.webp`: 1200 × 1600, lossless | 40 KB |
| `card/klar-card-loop.mp4`: 900 × 1200, H.264 CRF 14 | 174 KB |
| `card/klar-card-loop.webm`: 900 × 1200, VP9 CRF 18 | 171 KB |

The card plays through the same components as the video. Pill, GhostLine and
MailWindow take their timing as `Cues`; the video's cues are the defaults.
After that change, frames of both videos re-render pixel-identical to the
final masters.
