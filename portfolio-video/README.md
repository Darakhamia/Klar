# Klar — portfolio video

A silent 22-second loop of Klar for the portfolio site, rendered with
[Remotion](https://www.remotion.dev). [`STORYBOARD.md`](STORYBOARD.md) has the
storyboard, the decisions behind it, and what was changed after rendering.

The interface in the video is the app's own:

- **Real components, unchanged:** the overlay pill (`src/overlay/Overlay.tsx`),
  History and Voice (`src/sections/`), and the app's token and window CSS. They
  are imported straight from `../src`.
- **Fake data:** at bundle time `src/lib/settings.ts` is swapped for
  [`src/mocks/settings.ts`](src/mocks/settings.ts), so those components get demo
  data instead of calling Tauri.
- **Recreated:** only the settings window's frame, from `App.tsx`'s markup,
  because App will not run outside the Tauri shell.

This folder has its own `package.json`. Nothing here changes the app or its
dependencies.

## Run it

```sh
npm install
npm run studio     # preview and scrub the timeline in the browser
npm run render     # render everything into out/
npm run typecheck
```

`npm run render` (`scripts/render.sh`) does the following:

1. Draws lossless PNG masters into `build/`.
2. Checks that the first and last frames are identical, so the loop has no seam.
3. Encodes H.264 and VP9 with BT.709 colour tags and no audio.
4. Copies the two posters out of the masters.
5. Fails if any file is over 5 MB.

To experiment with quality, change the encoder settings with `H264_CRF=… VP9_CRF=…`.
Add `ENCODE_ONLY=1` to reuse the masters already in `build/` rather than drawing
them again.

For the browser, it uses a Playwright headless shell if the machine has one, or
`$REMOTION_BROWSER` if set. Otherwise Remotion downloads its own.

## Output

| File | What |
|---|---|
| `klar-16x9.mp4`, `klar-16x9.webm` | 1920 × 1080, 30 fps, H.264 and VP9 |
| `klar-1x1.mp4`, `klar-1x1.webm` | 1080 × 1080, the same story recomposed for square |
| `klar-poster.png`, `klar-poster-1x1.png` | The "Done" moment: clean text in the email, the pill reading Done |

`out/` is ignored here. The rendered files are committed only on the
`portfolio-video` branch, so about 13 MB of binaries never enters main's history.

## Where things are

| Path | What |
|---|---|
| `src/timeline.ts` | Every beat, in frames. Both formats share it |
| `src/format.ts` | The two layouts, and each one's camera path, framed on boxes measured from the real components |
| `src/speech.ts` | What is said and when; the waveform levels behind it |
| `src/ui/Pill.tsx` | The real overlay driven frame by frame. Its CSS transitions are redrawn per frame (`video.css`), because a frame-by-frame renderer has no clock for them |
| `src/scenes/` | The email, then History and Voice |
| `src/demo.ts` | The fake data. Nothing in it belongs to anyone |
