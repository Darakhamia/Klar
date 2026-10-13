/** Klar's own colours, from ../src/styles/tokens.css. */
export const RED = "#ec3013"; // --color-accent: Signal
export const RED_DARK_UI = "#ff563c"; // --color-accent-500: the red on dark UI
export const GROUND = "#f3f2f2"; // --color-bg
export const INK = "#201e1d"; // --color-text
export const WHITE = "#ffffff";
export const MUTED = "#7d7979"; // --color-neutral-600

export const FONT = '"Archivo", system-ui, sans-serif';

/** Archivo 800, −3 %, as the app's --text-display-xl. */
export const DISPLAY = {
  fontFamily: FONT,
  fontWeight: 800,
  letterSpacing: "-0.03em",
  lineHeight: 0.98,
} as const;
