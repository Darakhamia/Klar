# Klar — promo

A 28.6 s silent promo for the portfolio site, in 16:9 and 1:1. It loops
seamlessly through the end card. [`STORYBOARD.md`](STORYBOARD.md) has the beats,
the design rules and why each choice was made.

## The interface is captured, not drawn

The app's own frontend runs in Chromium and is captured with Playwright:

- **Running it.** The app's Vite dev server serves the real `index.html` and
  `overlay.html`. [`capture/bridge.js`](capture/bridge.js) stands in for the
  Tauri shell: `isTauri()` is true, every command is answered from
  [`capture/demo.mjs`](capture/demo.mjs), and engine events arrive as Rust
  sends them.
- **Resolution.** Shots are taken at device scale factor 6, so nothing is ever
  enlarged past its source pixels.
- **The pill.** The overlay is captured frame by frame. Its CSS transitions are
  paused and seeked to each frame's moment, so the motion is the stylesheet's
  own.
- **The video.** Remotion animates the captures. The type, the keycaps and the
  email (another app) are the video's own.

Nothing in `../src` is changed or copied. This folder has its own
`package.json`.

## Run it

```sh
npm install           # here, and once at the repository root for the app's Vite
npm run capture       # the app's frontend → public/captures/
npm run studio        # preview and scrub the timeline
npm run render        # both formats and posters into out/, checked with ffprobe
npm run typecheck
```

`npm run capture` starts the app's dev server on :1420 unless one is already
running.

`npm run render` (`scripts/render.sh`) does the following:

1. Draws lossless full-size PNG masters into `build/`.
2. Checks that the first and last frames are identical, so the loop has no seam.
3. Encodes H.264 High (yuv420p, CRF 18, faststart) and two-pass VP9, with
   BT.709 tags and no audio.
4. Copies the posters out of the masters.
5. Fails if any file is over 8 MB.
6. Prints each file's size, codec and bitrate as ffprobe reports them.

To change the encoder settings, set `H264_CRF=…` or `VP9_CRF=…`. Add
`ENCODE_ONLY=1` to reuse the masters already in `build/`.

## Output

| File | What |
|---|---|
| `klar-16x9.mp4`, `klar-16x9.webm` | 1920 × 1080, 30 fps |
| `klar-1x1.mp4`, `klar-1x1.webm` | 1080 × 1080, the same beats recomposed |
| `klar-poster.png`, `klar-poster-1x1.png` | Frame 300: the real pill at Done |

`out/` and `public/captures/` are ignored here. The rendered files are committed
only on the `portfolio-video` branch, so the binaries never enter main's
history.

## Where things are

| Path | What |
|---|---|
| `capture/capture.mjs` | Opens the app's pages, drives them, writes the captures and `manifest.json` |
| `capture/bridge.js` | The Tauri shell as far as the frontend can tell |
| `capture/demo.mjs`, `copy.json` | The demo data and the copy. Nothing in them belongs to anyone |
| `capture/speech.mjs` | Microphone levels for the dictation the pill listens to |
| `src/timeline.ts` | Every beat, in frames |
| `src/layout.ts` | Everything that differs between 16:9 and the square |
| `src/scenes/` | One file per beat |
| `src/captures.tsx` | How a capture is cropped, scaled and annotated |
