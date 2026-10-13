/**
 * Archivo, from @fontsource — the package the app ships its own copy from.
 * The first frame waits until every weight the video sets type in has loaded,
 * so no frame is ever drawn in a fallback face.
 */

import "@fontsource/archivo/400.css";
import "@fontsource/archivo/500.css";
import "@fontsource/archivo/600.css";
import "@fontsource/archivo/700.css";
import "@fontsource/archivo/800.css";
import { cancelRender, continueRender, delayRender } from "remotion";

const WEIGHTS = [400, 500, 600, 700, 800];
const SAMPLE = "Klar — You think faster. getklar.net";

const handle = delayRender("Loading Archivo");

Promise.all(WEIGHTS.map((weight) => document.fonts.load(`${String(weight)} 16px Archivo`, SAMPLE)))
  .then(() => {
    continueRender(handle);
  })
  .catch((error: unknown) => {
    cancelRender(error);
  });
