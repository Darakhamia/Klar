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
  /** The Processing row's first sentence is marked over `sweepFrames`. */
  sweep: 474,
  sweepFrames: 30,
  /** The settings window fades out on ink, then a cut to the light ground. */
  toEnd: 552,
  toLight: 566,
  endIn: 568,
  /** The poster: clean text in the email, the pill still reading Done. */
  poster: 270,
} as const;

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
