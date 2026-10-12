/**
 * The one timeline both formats share: 30 fps, 660 frames, 22 s.
 *
 * Frame numbers are absolute. The loop seam sits inside the end card's still
 * hold — frames 0 to 14 and the last frames are the same picture — so the
 * video can repeat forever without a visible join.
 */

export const FPS = 30;
export const DURATION = 660;

export const T = {
  /** End-card type starts to leave; the ground stays. */
  seamOut: 15,
  composeIn: 30,
  pillIn: 60,
  /** Key up. The real pipeline passes through Thinking here; the video does not. */
  release: 192,
  done: 225,
  /** The clean sentence lands word by word over `landFrames`. */
  land: 228,
  landFrames: 24,
  pillOut: 288,
  /** A straight cut to the dark History window, which then settles. */
  toDark: 330,
  /** Voice replaces History in one frame, as a click on the sidebar does. */
  toVoice: 447,
  /** The other History entries step back while the camera is on the newest. */
  listFocus: 352,
  listRelease: 426,
  /** Voice: everything but the Processing row steps back as the camera goes in. */
  voiceFocus: 466,
  /** The Processing row's first sentence is marked over `sweepFrames`, once
   * the camera has arrived on it. */
  sweep: 506,
  sweepFrames: 28,
  /** The settings window fades out on ink, then a cut to the light ground. */
  toEnd: 552,
  toLight: 566,
  endIn: 568,
  /** The poster: clean text in the email, the pill still reading Done. */
  poster: 270,
} as const;

/**
 * When the dictation's beats fall, in frames, and how the spoken words in
 * speech.ts map onto that time. The video and the portrait card play the same
 * dictation at different speeds, through the same components.
 */
export interface Cues {
  pillIn: number;
  release: number;
  done: number;
  land: number;
  landFrames: number;
  pillOut: number;
  /** Seconds of speech (speech.ts) at a second of this timeline: offset + rate × t. */
  speech: { offset: number; rate: number };
  /** The caret blinks from this frame, or holds steady when null. */
  blinkFrom: number | null;
  /** Where a loop must start empty: the frame the typed sentence clears. */
  clear: number | null;
}

export const VIDEO_CUES: Cues = {
  pillIn: T.pillIn,
  release: T.release,
  done: T.done,
  land: T.land,
  landFrames: T.landFrames,
  pillOut: T.pillOut,
  speech: { offset: 0, rate: 1 },
  blinkFrom: T.composeIn,
  clear: null,
};

/**
 * The portrait card: one dictation, 8 s, looping. Speech runs from 0.6 s to
 * 4.0 s, a little quicker than in the video, and the sentence clears at the
 * end so the loop starts and ends on the same empty email. The caret holds
 * steady so the last frame and the first are the same picture.
 */
export const CARD_DURATION = 240;
/** The still: Done, the sentence typed. */
export const CARD_STILL = 190;
export const CARD_CUES: Cues = {
  pillIn: 12,
  release: 126,
  done: 150,
  land: 152,
  landFrames: 24,
  pillOut: 196,
  speech: { offset: 2.3 - 0.6 * (3.88 / 3.4), rate: 3.88 / 3.4 },
  blinkFrom: null,
  clear: 204,
};

/** What was said, as the recogniser heard it. */
export const RAW = "hey so um the build is green now uh can you can you merge it before the standup";

/** What Klar typed. */
export const CLEAN = "Hey, the build is green now. Can you merge it before the standup?";

export interface Cue {
  text: string;
  from: number;
  to: number;
  tone: "light" | "dark";
}

/** Captions: four words at most, each arriving with the action it names. */
export const CUES: Cue[] = [
  { text: "Just speak.", from: 66, to: 184, tone: "light" },
  { text: "Klar cleans it up.", from: 198, to: 312, tone: "light" },
  { text: "Every word, kept.", from: 348, to: 436, tone: "dark" },
  { text: "Private. On-device.", from: 456, to: 545, tone: "dark" },
];
