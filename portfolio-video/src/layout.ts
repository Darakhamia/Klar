/**
 * The two formats. The beats and their timing are shared (timeline.ts); every
 * size and position that differs between 16:9 and the square lives here, so
 * the square is recomposed rather than cropped.
 */

export type Format = "wide" | "square";

export interface Layout {
  width: number;
  height: number;
  /** The left edge every headline hangs from. */
  margin: number;
  hook: number;
  raw: number;
  /** "No filler words." and "Proper punctuation." */
  benefit: number;
  /** The headline over a capture. */
  head: number;
  keys: { height: number; ctrl: number; space: number; label: number; plus: number; gap: number };
  /**
   * Frame pixels per CSS pixel of the pill, Listening and Done. The camera
   * pulls back as the pill widens from 260 to 360, so it keeps its place in
   * the frame. The capture is DPR 6: neither goes above that.
   */
  pill: { listening: number; done: number };
  email: { width: number; text: number; meta: number; top: number };
  /** Frame pixels per CSS pixel of the settings-window crops (DPR 6 source). */
  vocabulary: number;
  local: number;
  end: { mark: number; line: number; url: number };
}

export const LAYOUTS: Record<Format, Layout> = {
  wide: {
    width: 1920,
    height: 1080,
    margin: 150,
    hook: 184,
    raw: 150,
    benefit: 176,
    head: 104,
    keys: { height: 240, ctrl: 360, space: 860, label: 68, plus: 128, gap: 64 },
    pill: { listening: 5.4, done: 4.0 },
    email: { width: 1640, text: 92, meta: 40, top: 96 },
    vocabulary: 5,
    local: 4.8,
    end: { mark: 300, line: 104, url: 68 },
  },
  square: {
    width: 1080,
    height: 1080,
    margin: 96,
    hook: 136,
    raw: 112,
    benefit: 136,
    head: 84,
    keys: { height: 190, ctrl: 270, space: 520, label: 64, plus: 100, gap: 30 },
    pill: { listening: 3.4, done: 2.5 },
    email: { width: 940, text: 72, meta: 32, top: 96 },
    vocabulary: 2.8,
    local: 2.65,
    end: { mark: 240, line: 84, url: 64 },
  },
};
