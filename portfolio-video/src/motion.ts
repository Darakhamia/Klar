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

/** A camera position without a time: where it looks and how close. */
export type Framing = Omit<Shot, "frame">;

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface View {
  width: number;
  height: number;
}

/** `box` fills `fill` of the frame along its tighter side, centred. */
export function fit(view: View, box: Rect, fill: number): Framing {
  const zoom = Math.min((fill * view.width) / box.width, (fill * view.height) / box.height);
  return { x: box.x + box.width / 2, y: box.y + box.height / 2, zoom };
}

/** As `fit`, then raised so the box's bottom edge sits at `bottom` on screen —
 * clear of the captions, which own the bottom-left corner. */
export function fitAbove(view: View, box: Rect, fill: number, bottom: number): Framing {
  const { x, zoom } = fit(view, box, fill);
  return { x, y: box.y + box.height - (bottom - view.height / 2) / zoom, zoom };
}

/** Stage x `left`..`right` spans the frame exactly; stage y `top` is its top edge. */
export function span(view: View, left: number, right: number, top: number): Framing {
  const zoom = view.width / (right - left);
  return { x: (left + right) / 2, y: top + view.height / 2 / zoom, zoom };
}

/** `zoom` close, centred on `x`, with stage y `top` at the frame's top edge. */
export function from(view: View, x: number, top: number, zoom: number): Framing {
  return { x, y: top + view.height / 2 / zoom, zoom };
}

/** The same framing a touch closer, for a hold that should not look frozen. */
export const nudge = (framing: Framing, by = 1.012): Framing => ({ ...framing, zoom: framing.zoom * by });

export const at = (frame: number, framing: Framing): Shot => ({ frame, ...framing });

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
