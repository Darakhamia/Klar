/**
 * Stands in for `src/lib/settings.ts` at bundle time (see remotion.config.ts).
 *
 * The real module calls Tauri commands; this one answers the same calls with
 * the demo data, so History.tsx and Voice.tsx run unchanged. Only the exports
 * those two components use are here — anything else they reached for would
 * fail the bundle loudly rather than render something invented.
 */

import type { Acceleration, Dictation, PolishStatus } from "../../../src/lib/settings";
import { GPU, HISTORY, SETTINGS } from "../demo";

// The language list is the app's own. Imported by its file name so the
// replacement above does not swap it for this module.
export { LANGUAGES } from "../../../src/lib/settings.ts";

export const history = (limit: number): Promise<Dictation[]> =>
  Promise.resolve(HISTORY.slice(0, limit));

export const clearHistory = (): Promise<number> => Promise.resolve(0);

export const forgetDictation = (_id: number): Promise<boolean> => Promise.resolve(false);

export const acceleration = (): Promise<Acceleration> =>
  Promise.resolve({ accelerated: true, summary: GPU, warning: null, devices: [] });

export const polishStatus = (): Promise<PolishStatus> =>
  Promise.resolve({
    reachable: true,
    endpoint: SETTINGS.polishEndpoint,
    models: [SETTINGS.polishModel],
  });
