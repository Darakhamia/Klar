import { AbsoluteFill, Easing, useCurrentFrame } from "remotion";
import { Rise } from "../kinetic";
import type { Format, Layout } from "../layout";
import { ramp } from "../motion";
import { DISPLAY, GROUND, RED } from "../theme";

const LINES: Record<Format, string[]> = {
  wide: ["You think faster", "than you type."],
  square: ["You think", "faster than", "you type."],
};

/** The curtain: quick off the mark, settling as it clears the top. */
const lift = Easing.bezier(0.7, 0, 0.2, 1);

/** Beat 1, on red. At frame 0 nothing has risen yet: the loop's seam. */
export function Hook({ layout, format, liftAt }: { layout: Layout; format: Format; liftAt: number }) {
  const frame = useCurrentFrame();
  const lifted = ramp(frame, liftAt, 14, lift);
  return (
    <AbsoluteFill style={{ background: RED, transform: `translateY(${String(-lifted * 100)}%)` }}>
      <div
        style={{
          ...DISPLAY,
          position: "absolute",
          left: layout.margin,
          top: "50%",
          transform: "translateY(-50%)",
          fontSize: layout.hook,
          color: GROUND,
        }}
      >
        {LINES[format].map((line, index) => (
          <Rise key={line} text={line} start={4 + index * 10} />
        ))}
      </div>
    </AbsoluteFill>
  );
}
