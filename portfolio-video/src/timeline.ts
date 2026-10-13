/**
 * Every beat, in frames at 30 fps. STORYBOARD.md has the same table in words.
 * Both formats share it; only the layout differs.
 */

export const FPS = 30;

export const T = {
  hook: { from: 0, to: 78 },
  raw: { from: 78, to: 168 },
  keys: { from: 168, to: 207 },
  /** The captured pill: Listening frames play from `listen`, Done from `done`. */
  pill: { from: 207, to: 333, listen: 207, done: 279 },
  email: { from: 333, to: 423 },
  filler: { from: 423, to: 483 },
  punctuation: { from: 483, to: 543 },
  vocabulary: { from: 543, to: 648 },
  local: { from: 648, to: 753 },
  end: { from: 753, to: 858 },
} as const;

export const DURATION = T.end.to;

/** The pill at Done, settled: the poster. */
export const POSTER = 300;
