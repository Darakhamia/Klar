/**
 * What the Tauri side answers while the real frontend is captured.
 *
 * Every shape here mirrors a type in `../../src/lib/settings.ts` or
 * `../../src/lib/ipc.ts`. Nothing is anybody's: the address is on a domain
 * reserved for examples, the application names are generic, and the
 * dictations were written for the video.
 */

import copy from "../copy.json" with { type: "json" };

/** Unix seconds for a time today, local to whichever machine captures this. */
function today(hours, minutes, daysAgo = 0) {
  const at = new Date();
  at.setHours(0, 0, 0, 0);
  at.setDate(at.getDate() - daysAgo);
  at.setHours(hours, minutes);
  return Math.floor(at.getTime() / 1000);
}

export function demo(appearance) {
  const settings = {
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
    polishModel: "local-polish-4b",
    appearance,
    checkForUpdates: true,
    onboarded: true,
  };

  const dictionary = [
    { id: 1, term: "Kubernetes", replacements: ["cooper netties", "kuber nettis"], enabled: true },
    { id: 2, term: "PostgreSQL", replacements: ["postgres q l", "post gress"], enabled: true },
    { id: 3, term: "OAuth", replacements: ["oh auth", "o auth"], enabled: true },
    { id: 4, term: "Klar", replacements: ["clear", "klahr"], enabled: true },
  ];

  return {
    settings,
    version: { version: "0.3.2", os: "windows", arch: "x86_64" },
    models: [
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
    ],
    devices: [{ name: "USB Microphone", isDefault: true }],
    acceleration: { accelerated: true, summary: copy.gpu, warning: null, devices: [] },
    polish: { reachable: true, endpoint: settings.polishEndpoint, models: [settings.polishModel] },
    dictionary,
    history: [
      {
        id: 7,
        at: today(16, 42),
        text: copy.clean,
        raw: copy.raw,
        audioMs: 4300,
        latencyMs: 912,
        target: "Mail",
        polished: true,
      },
      {
        id: 6,
        at: today(15, 18),
        text: "Thanks for the notes. Let's move the review to Friday, and I'll send the deck the day before.",
        raw: "thanks for the notes uh let's move the review to thursday no actually friday and i'll send the deck the day before",
        audioMs: 6100,
        latencyMs: 1084,
        target: "Mail",
        polished: true,
      },
      {
        id: 5,
        at: today(14, 5),
        text: "Plan for tomorrow: finish the installer, then test the update on a clean machine.",
        raw: "okay so the plan for tomorrow is uh first finish the installer then um test the update on a clean machine",
        audioMs: 5200,
        latencyMs: 1196,
        target: "Notepad",
        polished: true,
      },
      {
        id: 4,
        at: today(11, 30),
        text: "Rename the helper to parseConfig and add a test for empty input.",
        raw: "rename the helper to parse config and um add a test for empty input",
        audioMs: 3800,
        latencyMs: 590,
        target: "Code",
        polished: true,
      },
    ],
    stats: {
      totals: { dictations: 412, words: 18_930, chars: 104_200, audioMs: 7_560_000, days: 30 },
      daily: [],
    },
  };
}

/** What the dictionary does to a line — whole words, longest phrase first,
 * case-insensitive — for the editor's live preview. */
export function applyDictionary(entries, text) {
  const rules = entries
    .filter((entry) => entry.enabled)
    .flatMap((entry) => entry.replacements.map((heard) => ({ heard, term: entry.term })))
    .sort((a, b) => b.heard.length - a.heard.length);
  let out = text;
  for (const { heard, term } of rules) {
    const escaped = heard.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    out = out.replace(new RegExp(`\\b${escaped}\\b`, "gi"), term);
  }
  return out;
}
