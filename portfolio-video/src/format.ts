/**
 * The two compositions. Same story, laid out twice: the square is recomposed
 * — tighter windows, its own camera paths — rather than cropped from 16:9.
 *
 * Everything is in stage pixels: a logical canvas drawn at 1.5×, so the app's
 * own sizes (a 260 × 64 pill, a 900 × 620 window) stay in true proportion.
 */

import type { Shot } from "./motion";

export type Format = "wide" | "square";

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Layout {
  width: number;
  height: number;
  /** Stage pixels to output pixels. */
  scale: number;
  /** The one left edge captions and the end card share. */
  margin: number;
  captionBottom: number;
  mail: Box;
  /** Width the ghost line may take before it wraps. */
  ghostWidth: number;
  composeShots: readonly Shot[];
  /** Top-left of the settings window, title bar included. */
  window: { x: number; y: number };
  settingsShots: readonly Shot[];
  wordmark: number;
}

/** `overlay.rs`: the overlay sits 12 % of the screen height above the bottom. */
export const pillTop = (layout: Layout): number => Math.round(layout.height * 0.88 - 64);

export const LAYOUTS: Record<Format, Layout> = {
  wide: {
    width: 1280,
    height: 720,
    scale: 1.5,
    margin: 64,
    captionBottom: 44,
    mail: { x: 230, y: 80, width: 820, height: 380 },
    ghostWidth: 900,
    composeShots: [
      { frame: 30, x: 640, y: 360, zoom: 1 },
      { frame: 192, x: 640, y: 372, zoom: 1.04 },
      { frame: 225, x: 640, y: 440, zoom: 1.12 },
      { frame: 262, x: 640, y: 352, zoom: 1.12 },
      { frame: 336, x: 640, y: 340, zoom: 1.15 },
    ],
    window: { x: 380, y: 72 },
    // The window keeps clear of the caption: its left edge, then the sidebar's
    // rule once the camera is closer, stays right of where captions end.
    settingsShots: [
      { frame: 330, x: 650, y: 360, zoom: 0.98 },
      { frame: 444, x: 600, y: 300, zoom: 1.2 },
      { frame: 462, x: 600, y: 300, zoom: 1.2 },
      { frame: 534, x: 795, y: 300, zoom: 1.33 },
      { frame: 566, x: 800, y: 298, zoom: 1.35 },
    ],
    wordmark: 150,
  },
  square: {
    width: 720,
    height: 720,
    scale: 1.5,
    margin: 44,
    captionBottom: 32,
    mail: { x: 40, y: 84, width: 640, height: 370 },
    ghostWidth: 600,
    composeShots: [
      { frame: 30, x: 360, y: 360, zoom: 1 },
      { frame: 192, x: 360, y: 372, zoom: 1.04 },
      { frame: 225, x: 360, y: 440, zoom: 1.1 },
      { frame: 262, x: 360, y: 372, zoom: 1.08 },
      { frame: 336, x: 360, y: 362, zoom: 1.1 },
    ],
    window: { x: 40, y: 36 },
    // Too narrow for the sidebar and a caption side by side: the sidebar stays
    // just out of frame, and the window stays above a band of ink at the
    // bottom where the caption sits.
    settingsShots: [
      { frame: 330, x: 652, y: 400, zoom: 0.88 },
      { frame: 444, x: 640, y: 396, zoom: 0.92 },
      { frame: 462, x: 640, y: 396, zoom: 0.92 },
      { frame: 534, x: 604, y: 410, zoom: 1 },
      { frame: 566, x: 606, y: 411, zoom: 1.002 },
    ],
    wordmark: 128,
  },
};
