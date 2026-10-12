/**
 * Klar, 22 seconds, silent, looping.
 *
 * The app's own stylesheets come first so the interface draws exactly as it
 * does in the shell; video.css only adds what surrounds it.
 */

import "../../src/styles/global.css";
import "../../src/App.css";
import "./video.css";
import "./fonts";

import { AbsoluteFill, useCurrentFrame } from "remotion";
import { LAYOUTS, type Format } from "./format";
import { ComposeScene } from "./scenes/ComposeScene";
import { SettingsScene } from "./scenes/SettingsScene";
import { T } from "./timeline";
import { Captions } from "./ui/Captions";
import { EndCard } from "./ui/EndCard";
import { Theme } from "./ui/Theme";

export function Video({ format }: { format: Format }) {
  const frame = useCurrentFrame();
  const layout = LAYOUTS[format];
  const dark = frame >= T.toDark && frame < T.toLight;

  return (
    <AbsoluteFill className="kv" style={{ background: "var(--kv-ground)" }}>
      <Theme dark={dark} />
      <div
        className="kv-stage"
        style={{ width: layout.width, height: layout.height, transform: `scale(${String(layout.scale)})` }}
      >
        {frame >= T.composeIn && frame < T.toDark && <ComposeScene layout={layout} />}
        <EndCard layout={layout} />
        {dark && <SettingsScene layout={layout} />}
        <Captions layout={layout} />
      </div>
    </AbsoluteFill>
  );
}
