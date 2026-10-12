/**
 * Holds the first frame until every Archivo weight the interface uses has
 * loaded — the faces come from @fontsource, the same package the app ships,
 * through global.css. Without this a frame can be drawn in a fallback face.
 */

import { cancelRender, continueRender, delayRender } from "remotion";

const WEIGHTS = [400, 500, 600, 700, 800];
const SAMPLE = "Klar — 0:04 · Speak.";

const handle = delayRender("Loading Archivo");

Promise.all(WEIGHTS.map((weight) => document.fonts.load(`${String(weight)} 16px Archivo`, SAMPLE)))
  .then(() => {
    continueRender(handle);
  })
  .catch((error: unknown) => {
    cancelRender(error);
  });
