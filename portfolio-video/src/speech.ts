/**
 * The dictation the video performs: what is said, when, and how loudly.
 *
 * The real overlay is fed one peak level every 50 ms by the engine. The
 * waveform here is fed the same way, from a speech envelope that follows the
 * ghost line's words syllable by syllable, so the bars move with the words.
 */

import { FPS, VIDEO_CUES, type Cues } from "./timeline";

export interface Word {
  text: string;
  /** Seconds from the start of the video. */
  start: number;
  length: number;
  syllables: number;
  /** Removed by the polish: the fillers and the false start. */
  filler: boolean;
}

const word = (text: string, start: number, length: number, syllables = 1, filler = false): Word => ({
  text,
  start,
  length,
  syllables,
  filler,
});

/** The raw transcript, word by word. Joined with spaces it is `RAW`. */
export const SPOKEN: Word[] = [
  word("hey", 2.3, 0.2),
  word("so", 2.52, 0.14, 1, true),
  word("um", 2.72, 0.26, 1, true),
  word("the", 3.12, 0.08),
  word("build", 3.22, 0.2),
  word("is", 3.44, 0.1),
  word("green", 3.56, 0.22),
  word("now", 3.8, 0.24),
  word("uh", 4.22, 0.22, 1, true),
  word("can", 4.56, 0.12, 1, true),
  word("you", 4.7, 0.1, 1, true),
  word("can", 4.82, 0.12),
  word("you", 4.96, 0.1),
  word("merge", 5.08, 0.22),
  word("it", 5.32, 0.08),
  word("before", 5.42, 0.26, 2),
  word("the", 5.7, 0.08),
  word("standup", 5.8, 0.38, 2),
];

/** The engine's level cadence: one peak per 50 ms of audio. */
const TICK = 0.05;
/** `overlay.css`: bar height follows the level over 130 ms, linear. */
const FOLLOW = 0.13;
/** `useEngine.ts`: twelve bars, newest last. */
const BARS = 12;
/** `Overlay.tsx`'s own mapping from level to bar height. */
const height = (level: number): number => Math.max(3, Math.sqrt(level) * 30);

/** Deterministic noise in 0..1, so every render of a frame is the same frame. */
function noise(n: number): number {
  const x = Math.sin(n * 12.9898 + 78.233) * 43758.5453;
  return x - Math.floor(x);
}

/** The peak the engine would report for the 50 ms ending at `seconds`. */
function peak(seconds: number, tick: number): number {
  let level = 0.004 + 0.008 * noise(tick);
  SPOKEN.forEach((spoken, index) => {
    const per = spoken.length / spoken.syllables;
    for (let syllable = 0; syllable < spoken.syllables; syllable++) {
      const centre = spoken.start + per * (syllable + 0.5);
      const distance = Math.abs(seconds - centre) / (per / 2 + 0.06);
      if (distance < 1) {
        const loudness = (spoken.filler ? 0.34 : 0.62) * (0.8 + 0.3 * noise(index * 7 + syllable));
        level = Math.max(level, loudness * (0.5 + 0.5 * Math.cos(Math.PI * distance)));
      }
    }
  });
  return Math.min(1, level * (0.85 + 0.3 * noise(tick + 1000)));
}

/**
 * Each bar's drawn height, `t` seconds after recording began.
 *
 * The CSS transition is worked out here rather than left to the browser: a
 * renderer that captures one frame at a time has no clock for a transition to
 * run against. Every level event restarts each bar's 130 ms transition from
 * wherever it had got to, exactly as the browser does.
 */
export function barHeights(t: number, cues: Cues = VIDEO_CUES): number[] {
  const from: number[] = new Array<number>(BARS).fill(3);
  const to: number[] = new Array<number>(BARS).fill(3);
  const peaks: number[] = [];
  let changed = 0;

  for (let tick = 0; tick * TICK <= t; tick++) {
    const at = tick * TICK;
    const progress = Math.min(1, (at - changed) / FOLLOW);
    peaks.push(peak(cues.speech.offset + cues.speech.rate * (cues.pillIn / FPS + at), tick));
    for (let bar = 0; bar < BARS; bar++) {
      const start = from[bar] ?? 3;
      from[bar] = start + ((to[bar] ?? 3) - start) * progress;
      const source = peaks[peaks.length - BARS + bar];
      to[bar] = source === undefined ? 3 : height(source);
    }
    changed = at;
  }

  const progress = Math.min(1, (t - changed) / FOLLOW);
  return from.map((start, bar) => start + ((to[bar] ?? 3) - start) * progress);
}

/** The frame, on a timeline with these cues, at which a word starts being said. */
export const spokenAt = (word: Word, cues: Cues = VIDEO_CUES): number =>
  Math.round(((word.start - cues.speech.offset) / cues.speech.rate) * FPS);

/** The level that makes `Overlay.tsx` draw a bar `pixels` tall. */
export const levelFor = (pixels: number): number => (pixels <= 3 ? 0 : (pixels / 30) ** 2);
