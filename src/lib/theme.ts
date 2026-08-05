/**
 * Which of the two shells a window is drawn in.
 *
 * The tokens do all the work — `tokens.css` defines light at `:root`, dark
 * under `[data-theme="dark"]`, and follows the OS when neither is pinned. This
 * only decides which of those three the document is in.
 */

import type { Appearance } from "./settings";

export function applyAppearance(appearance: Appearance): void {
  const root = document.documentElement;
  if (appearance === "system") {
    // Removed rather than set to a value: the `prefers-color-scheme` rule in
    // tokens.css is written for the absence of the attribute, so the window
    // follows the OS live rather than being told once at startup.
    delete root.dataset.theme;
  } else {
    root.dataset.theme = appearance;
  }
}
