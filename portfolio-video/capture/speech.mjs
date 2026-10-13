/**
 * Microphone peaks for the dictation the overlay is captured listening to.
 *
 * The engine sends one `level` event per 50 ms of audio, carrying the peak
 * sample of that stretch, 0..1 (src-tauri/src/engine.rs). This produces the
 * same: a syllable-shaped envelope for the raw sentence at a natural speaking
 * rate, a quiet floor between words, and a fixed seed so every capture is the
 * same take.
 */

import copy from "../copy.json" with { type: "json" };

export const LEVEL_MS = 50;

function random(seed) {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Rough syllable count, enough to shape a word's envelope. */
const syllables = (word) => Math.max(1, (word.match(/[aeiouy]+/g) ?? []).length);

/** `count` peaks, one per 50 ms, starting at the moment recording starts. */
export function levelsFor(count) {
  const rand = random(7);
  const fillers = new Set(["so", "um", "uh"]);
  const peaks = [];
  // A breath before the first word.
  for (let i = 0; i < 4; i++) peaks.push(0.004 + rand() * 0.006);
  for (const word of copy.raw.split(" ")) {
    const quiet = fillers.has(word) ? 0.45 : 1;
    for (let s = 0; s < syllables(word); s++) {
      // A close microphone: syllables peak near full scale, as in the real
      // screenshot, where most bars stand close to their 30 px.
      const top = (0.45 + rand() * 0.55) * quiet;
      // Attack, peak, release: 150 ms a syllable, with a dip after it.
      for (const shape of [0.55, 1, 0.7]) peaks.push(Math.min(1, top * shape * (0.85 + rand() * 0.3)));
      peaks.push(0.03 + rand() * 0.08);
    }
    // Words run together more often than not; sometimes there is a pause.
    if (rand() > 0.6) peaks.push(0.004 + rand() * 0.01, 0.004 + rand() * 0.01);
  }
  while (peaks.length < count) peaks.push(0.004 + rand() * 0.006);
  return peaks.slice(0, count);
}
