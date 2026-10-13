import { AbsoluteFill, Img, staticFile, useCurrentFrame } from "remotion";
import { useManifest } from "../captures";
import type { Layout } from "../layout";
import { ramp, resolve } from "../motion";
import { GROUND } from "../theme";
import { Float } from "./Card";

/**
 * Beats 3b and 3c: the real overlay, captured frame by frame. Listening plays
 * from the first frame of the beat; its last captured frame runs straight into
 * Done, so the sequence plays through untouched and then holds.
 */
export function Pill({ layout, duration }: { layout: Layout; duration: number }) {
  const frame = useCurrentFrame();
  const { pill } = useManifest();
  const index = Math.min(frame, pill.frames - 1);
  const width = pill.widths[index] ?? 360;
  const name = `${pill.dir}/${String(index).padStart(4, "0")}.png`;
  // Pull back while the pill widens: 420 ms on the app's own curve, which is
  // how long and how the stylesheet animates the width.
  const widened = ramp(frame, pill.done.from, 13, resolve);
  const scale = layout.pill.listening + (layout.pill.done - layout.pill.listening) * widened;
  return (
    <AbsoluteFill style={{ background: GROUND, alignItems: "center", justifyContent: "center" }}>
      <Float duration={duration} tilt={14} push={0.05}>
        <Img
          src={staticFile(`captures/${name}`)}
          style={{ display: "block", width: width * scale, height: pill.height * scale }}
        />
      </Float>
    </AbsoluteFill>
  );
}
