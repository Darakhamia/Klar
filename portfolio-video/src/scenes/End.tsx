import { AbsoluteFill, useCurrentFrame } from "remotion";
import copy from "../../copy.json";
import { Rise } from "../kinetic";
import type { Layout } from "../layout";
import { ramp, resolve } from "../motion";
import { DISPLAY, FONT, GROUND, INK, RED } from "../theme";

/**
 * Beat 7, on red. In its last frames everything rises out of its mask and the
 * frame is plain red — exactly the first frame, so the loop has no seam.
 */
export function End({ layout, clearAt }: { layout: Layout; clearAt: number }) {
  const frame = useCurrentFrame();
  const e = layout.end;
  const out = ramp(frame, clearAt, 11, resolve);
  const leaving = { transform: `translateY(${String(-out * 120)}%)` };
  const mask = { overflow: "hidden", padding: "0.06em 0 0.16em", margin: "-0.06em 0 -0.16em" };
  return (
    <AbsoluteFill style={{ background: RED, justifyContent: "center" }}>
      <div style={{ marginLeft: layout.margin }}>
        <div style={{ ...mask, ...DISPLAY, fontSize: e.mark, color: GROUND, letterSpacing: "-0.04em" }}>
          <div style={leaving}>
            <Rise text="Klar" start={0} frames={18} />
          </div>
        </div>
        <div style={{ ...mask, ...DISPLAY, fontSize: e.line, color: GROUND, marginTop: e.line * 0.25 }}>
          <div style={leaving}>
            <Rise text="Speak. Get clean text." start={8} />
          </div>
        </div>
        <div
          style={{
            ...mask,
            fontFamily: FONT,
            fontWeight: 700,
            fontSize: e.url,
            letterSpacing: "0.02em",
            color: INK,
            marginTop: e.line * 0.7,
          }}
        >
          <div style={leaving}>
            <Rise text={copy.site} start={18} />
          </div>
        </div>
      </div>
    </AbsoluteFill>
  );
}
