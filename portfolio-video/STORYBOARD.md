# Klar — promo storyboard

A 28.6 s silent promo for the portfolio site. It loops seamlessly through the
end card. It replaces the earlier 22 s walkthrough, which read as a screen
recording: the UI was too small, the framing was off, and nothing was sold.

This one is an ad. Each beat shows one thing, big: either a line of type or a
single piece of the real interface, cropped so that it fills the frame.

## How the interface gets on screen

The interface is never redrawn. It is the app's own frontend, captured
(`capture/capture.mjs`):

- **Running it.** The app's Vite dev server serves the real `index.html` and
  `overlay.html`. Chromium opens them with `capture/bridge.js` standing in for
  the Tauri shell. `isTauri()` is true, every `invoke` is answered from
  `capture/demo.mjs`, and engine events arrive the way Rust sends them. Nothing
  in `../src` is changed or copied.
- **Resolution.** Shots are taken with Playwright at device scale factor 6, so
  that nothing on screen is ever enlarged past its source pixels, even with the
  pill filling three quarters of a 1080p frame. Side-by-side checks against the
  real app use DPR 2.
- **The pill.** The overlay is captured as a 96-frame sequence at 30 fps. The
  page runs on a virtual clock. Each frame, the engine events that are due get
  delivered (`recording`, a `level` every 50 ms as the engine sends them, then
  the text and `injecting`). Every CSS transition the stylesheet starts is then
  paused and seeked to that frame's moment. The bar heights, the 260 → 360 px
  width change, the bars collapsing and the colour fades are all the app's own
  transitions, drawn frame by frame.
- **Checked first.** Three captures were put next to your screenshots of the
  real app before anything was built: History (dark), Voice (light) and the
  overlay (Listening, Done).

Everything else is the video's own and is drawn in Remotion:

- the type
- the keycaps
- the email the text lands in (another app, not Klar)

## Design

- **Colour.** Klar's own: red `#EC3013`, ground `#F3F2F2`, ink `#201E1D`.
- **Type.** Archivo throughout. Headlines are Archivo 800 with −3 % tracking,
  as the app's display style. Every line of type is 2–5 words and at least
  64 px at 1080p (most are 96–150 px), and holds for at least a second.
- **Shapes.** Zero radius and 2 px rules, scaled with the type.
- **Captures.** They float on a clean ground with a soft shadow, a slow
  push-in and a slight tilt that settles flat.
- **Motion.** The app's curve, `--ease-resolve`: things resolve and never
  bounce.
- **Pace.** 1.3–3.5 s a beat.

## Beats

Frames are at 30 fps.

| # | Frames | Time | Ground | What is on screen |
|---|---|---|---|---|
| 1 | 0–78 | 0.0–2.6 | Red | **"You think faster / than you type."** Two lines, word by word, rising out of a mask. |
| 2 | 78–168 | 2.6–5.6 | Ground | The red lifts away like a curtain. The raw transcript, *"hey so um the build is green now uh can you can you…"*, arrives word by word at speaking pace in 150 px lines that scroll up as they fill, with a red caret. The filler words are red. |
| 3a | 168–207 | 5.6–6.9 | Ground | Two keycaps, **CTRL + SPACE**, drop in and are pressed. |
| 3b | 207–279 | 6.9–9.3 | Ground | The press cuts to the **real overlay pill**, at 73 % of the frame's width: Listening, bars live, the counter running 0:01 → 0:03. |
| 3c | 279–333 | 9.3–11.1 | Ground | Key released: the pill turns to **Done**. The width grows, the bars collapse to grey, and the clean sentence appears. The camera pulls back as the pill widens, so it keeps its place in the frame. |
| 4 | 333–423 | 11.1–14.1 | Ground | An email in close-up. *"Hey, the build is green now. Can you merge it before the standup?"* lands word by word in 0.8 s, highlighted as it is inserted. |
| 5a | 423–483 | 14.1–16.1 | Ink | **"Um, uh, no filler words."** The "Um, uh," is struck through and drops out, leaving **"No filler words."** |
| 5b | 483–543 | 16.1–18.1 | Red | **"proper punctuation"** gets its capital P and its full stop: **"Proper punctuation."** |
| 5c | 543–648 | 18.1–21.6 | Ground | **"Your own vocabulary."** over the **real Dictionary screen**. "deploy it to cooper netties…" becomes "deploy it to **Kubernetes**…" in the live preview, underlined in red. |
| 6 | 648–753 | 21.6–25.1 | Ground | **"Runs on your computer. / Nothing leaves it."** over the **real Voice panel**. *"Speech is recognised on this machine and never leaves it."* is underlined in red. |
| 7 | 753–858 | 25.1–28.6 | Red | End card: **Klar**, **"Speak. Get clean text."**, **getklar.net**. In the last 12 frames it clears to plain red: the first frame, so the loop has no seam. |

The poster is frame 300: the pill at Done, the strongest product frame.

## Square (1080 × 1080)

The square uses the same beats and timing, recomposed rather than cropped:

- The headlines break onto more lines, at 84–136 px.
- The keycaps sit closer together.
- The pill fills 85 % of the width.
- The email is narrower, so the sentence takes three lines.
- The Dictionary and Voice crops are re-cut to fit a square card.

## Demo data

Nothing in the video is anybody's:

- The email goes to `team@example.com`.
- The History targets are generic: Mail, Notepad.
- The dictionary terms are technical jargon.
- The GPU is the demo's RTX 4060.

## Notes from the capture check

- **The overlay doesn't load Archivo.** Only the settings and onboarding
  windows import `@fontsource/archivo`. Your screenshots show the pill in
  Archivo, so the capture gives the overlay page the same font files. On a
  machine without Archivo installed, the real pill falls back to the system
  font.
- **The capture indents lists by 40 px.** `.hist` and `.dict` remove the list
  bullets but not the browser's default list padding, so the captured
  History and Dictionary lists sit 40 px in from their headers. Your History
  screenshot shows them flush. None of the video's crops include a list, so
  this doesn't change any frame.
