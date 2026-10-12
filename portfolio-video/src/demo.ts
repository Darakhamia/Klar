/**
 * The fake data the video shows. Nothing here is anybody's: the address is on
 * a domain reserved for examples, the application names are generic, and the
 * dictations were written for the video.
 */

import type { Device, Dictation, ModelStatus, Settings } from "../../src/lib/settings";
import { CLEAN, RAW } from "./timeline";

export const MAIL = {
  to: "team@example.com",
  subject: "Today's release",
};

export const VERSION = "0.3.2 · windows x86_64";

export const SETTINGS: Settings = {
  hotkey: { modifiers: ["control"], key: "space" },
  launchAtLogin: true,
  onFinish: "type",
  overlayPosition: "bottomCentre",
  language: "en",
  processing: "local",
  model: "large-v3-turbo-q5_0",
  microphone: null,
  cleanup: "balanced",
  accuracy: "fast",
  polishEndpoint: "http://127.0.0.1:11434",
  polishModel: "qwen3-4b-instruct-q4",
  appearance: "dark",
  checkForUpdates: true,
  onboarded: true,
};

/** The two speech entries of the real catalogue, as the settings window lists them. */
export const MODELS: ModelStatus[] = [
  {
    id: "large-v3-turbo-q5_0",
    kind: "speech",
    fileName: "ggml-large-v3-turbo-q5_0.bin",
    url: "",
    sha256: "",
    bytes: 574_041_195,
    multilingual: true,
    summary:
      "Default. Multilingual, quantised to a third of the size. Quantisation costs accuracy, and it costs it first on the words a dictionary exists for: names, jargon, anything rare.",
    installed: true,
    size: "547 MB",
  },
  {
    id: "large-v3-turbo",
    kind: "speech",
    fileName: "ggml-large-v3-turbo.bin",
    url: "",
    sha256: "",
    bytes: 1_624_555_275,
    multilingual: true,
    summary:
      "Unquantised. Better on unfamiliar words, three times the memory and the download. Worth it on a machine with a graphics card to spare.",
    installed: false,
    size: "1549 MB",
  },
];

export const DEVICES: Device[] = [{ name: "USB Microphone", isDefault: true }];

export const GPU = "NVIDIA GeForce RTX 4060 (cuda)";

/** Unix seconds for a time today, local to whichever machine renders this. */
function today(hours: number, minutes: number, daysAgo = 0): number {
  const at = new Date();
  at.setHours(0, 0, 0, 0);
  at.setDate(at.getDate() - daysAgo);
  at.setHours(hours, minutes);
  return Math.floor(at.getTime() / 1000);
}

/** Newest first, as the store returns them. The first is the one the video dictates. */
export const HISTORY: Dictation[] = [
  {
    id: 4,
    at: today(9, 41),
    text: CLEAN,
    raw: RAW,
    audioMs: 4300,
    latencyMs: 812,
    target: "Mail",
    polished: true,
  },
  {
    id: 3,
    at: today(9, 12),
    text: "Move the review to Friday and I'll write up the notes after.",
    raw: "move the review to thursday no friday and uh i'll write up the notes after",
    audioMs: 4100,
    latencyMs: 640,
    target: "Notepad",
    polished: true,
  },
  {
    id: 2,
    at: today(8, 57),
    text: "Rename the helper to parseConfig and add a test for empty input.",
    raw: "rename the helper to parse config and um add a test for empty input",
    audioMs: 3800,
    latencyMs: 590,
    target: "Code",
    polished: true,
  },
  {
    id: 1,
    at: today(18, 5, 1),
    text: "Thanks, that works for me. Let's pick it up tomorrow.",
    raw: "thanks that works for me uh let's pick it up tomorrow",
    audioMs: 3200,
    latencyMs: 701,
    target: "Mail",
    polished: true,
  },
];
