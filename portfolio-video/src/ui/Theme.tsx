import { useLayoutEffect } from "react";

/**
 * The settings window takes its dark theme the way the app gives it one: from
 * the root element, through tokens.css. Everything else in the video is
 * coloured explicitly, so only that window follows this.
 */
export function Theme({ dark }: { dark: boolean }) {
  useLayoutEffect(() => {
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  }, [dark]);

  return null;
}
