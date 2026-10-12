/**
 * Easing and camera.
 *
 * The interface's own motion uses the app's curve, `--ease-resolve` — resolve,
 * never bounce. The camera is the video's alone: slow moves that start and
 * stop at rest, with nothing that overshoots.
 */

import { Easing, interpolate } from "remotion";

/** `--ease-resolve` from tokens.css. */
export const resolve = Easing.bezier(0.2, 0.8, 0.2, 1);

/** For the camera: leaves rest gently and arrives at rest gently. */
const drift = Easing.bezier(0.45, 0, 0.25, 1);

const linear = (t: number): number => t;

/** 0 → 1 across `frames` frames from `start`, held at either end. */
export function ramp(frame: number, start: number, frames: number, easing = linear): number {
  return interpolate(frame, [start, start + frames], [0, 1], {
    easing,
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
}

/** Where the camera looks, in stage pixels, and how close. */
export interface Shot {
  frame: number;
  x: number;
  y: number;
  zoom: number;
}

/** The camera at `frame`, eased between the shots either side of it. */
export function aim(frame: number, shots: readonly Shot[]): Shot {
  const first = shots[0];
  if (first === undefined) return { frame, x: 0, y: 0, zoom: 1 };
  let from = first;
  for (const shot of shots) {
    if (shot.frame > frame) {
      const t = drift(Math.min(1, (frame - from.frame) / Math.max(1, shot.frame - from.frame)));
      return {
        frame,
        x: from.x + (shot.x - from.x) * t,
        y: from.y + (shot.y - from.y) * t,
        zoom: from.zoom + (shot.zoom - from.zoom) * t,
      };
    }
    from = shot;
  }
  return { ...from, frame };
}
