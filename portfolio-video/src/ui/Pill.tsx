/**
 * The real overlay, driven by the timeline instead of the engine.
 *
 * `Overlay.tsx` renders from an `Engine` value and nothing else, so it runs
 * here unchanged: the video hands it the states, levels and text the engine
 * would have sent. What the video adds is time — the app's CSS transitions are
 * replaced by the same durations and curve computed per frame (video.css).
 */

import type { CSSProperties } from "react";
import { useCurrentFrame } from "remotion";
import { Overlay } from "../../../src/overlay/Overlay";
import type { Engine, PipelineState } from "../../../src/overlay/useEngine";
import type { Layout } from "../format";
import { ramp, resolve } from "../motion";
import { barHeights, levelFor } from "../speech";
import { CLEAN, FPS, RAW, T } from "../timeline";

/** tokens.css: --duration-collapse, --duration-state; overlay.css: bar height. */
const COLLAPSE = 0.42 * FPS;
const STATE = 0.22 * FPS;
const FALL = 0.13 * FPS;

const RECORDED = (T.release - T.pillIn) / FPS;

export function Pill({ layout }: { layout: Layout }) {
  const frame = useCurrentFrame();
  if (frame < T.pillIn || frame > T.pillOut + 10) return null;

  const state: PipelineState =
    frame < T.release ? "recording" : frame < T.done ? "polishing" : "injecting";
  const elapsed = Math.min((frame - T.pillIn) / FPS, RECORDED);
  const live = barHeights(elapsed);

  const engine: Engine = {
    state,
    levels: live.map(levelFor),
    text: state === "recording" ? "" : state === "polishing" ? RAW : CLEAN,
    error: null,
    elapsed,
  };

  // Frames since the key came up: the pill widens, the bars flatten and grey.
  const since = frame - T.release;
  const widen = since < 0 ? 0 : resolve(Math.min(1, since / COLLAPSE));
  const grey = since < 0 ? 0 : Math.min(1, since / STATE);
  const falling = since >= 0 && since < FALL;
  const fall = Math.min(1, Math.max(0, since / FALL));

  const vars: Record<string, string> = {
    "--kv-pill-width": `${String(260 + 100 * widen)}px`,
    "--kv-bar-width": `${String(3 + 2 * widen)}px`,
    "--kv-bar-colour": `color-mix(in srgb, var(--color-neutral-600) ${String(grey * 100)}%, var(--color-accent-500))`,
    "--kv-bar-opacity": String(1 - 0.55 * grey),
    "--kv-status-colour": `color-mix(in srgb, var(--color-neutral-400) ${String(grey * 100)}%, var(--color-accent-500))`,
  };
  if (falling) {
    live.forEach((pixels, bar) => {
      vars[`--kv-h${String(bar)}`] = `${String(pixels + (3 - pixels) * fall)}px`;
    });
  }

  const enter = ramp(frame, T.pillIn, 12, resolve);
  const leave = ramp(frame, T.pillOut, 8, resolve);

  return (
    <div
      className="kv-pill-slot"
      style={{
        top: layout.pillTop,
        width: layout.width,
        opacity: enter * (1 - leave),
        transform: `translateY(${String(16 * (1 - enter) + 8 * leave)}px)`,
      }}
    >
      <div className={falling ? "kv-pill kv-pill--falling" : "kv-pill"} style={vars as CSSProperties}>
        <Overlay engine={engine} />
      </div>
    </div>
  );
}
