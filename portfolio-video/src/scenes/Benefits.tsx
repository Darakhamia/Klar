import { AbsoluteFill, useCurrentFrame } from "remotion";
import { Rise } from "../kinetic";
import type { Format, Layout } from "../layout";
import { ramp, resolve } from "../motion";
import { DISPLAY, GROUND, INK, RED } from "../theme";

/**
 * A letter that changes in place: it rises in with its word at `enter`, then
 * at `at` drops out as its replacement drops in.
 */
function Swap({ from, to, enter, at, frame }: { from: string; to: string; enter: number; at: number; frame: number }) {
  const shown = ramp(frame, enter, 16, resolve);
  const t = ramp(frame, at, 8, resolve);
  return (
    <span
      style={{
        display: "inline-block",
        position: "relative",
        overflow: "hidden",
        verticalAlign: "top",
        padding: "0.06em 0 0.14em",
        margin: "-0.06em 0 -0.14em",
      }}
    >
      <span style={{ display: "inline-block", transform: `translateY(${String((1 - shown) * 110 + t * 110)}%)` }}>
        {from}
      </span>
      <span style={{ position: "absolute", left: 0, top: "0.06em", transform: `translateY(${String((t - 1) * 110)}%)` }}>
        {to}
      </span>
    </span>
  );
}

/** Beat 5a, on ink: "Um, uh," is struck out and falls away. */
export function Filler({ layout, format }: { layout: Layout; format: Format }) {
  const frame = useCurrentFrame();
  const strike = ramp(frame, 18, 8, resolve);
  // Out first, then the line closes up.
  const faded = ramp(frame, 27, 6);
  const gone = ramp(frame, 31, 13, resolve);
  const rest = format === "wide" ? ["filler words."] : ["filler", "words."];
  const size = layout.benefit;
  return (
    <AbsoluteFill style={{ background: INK, justifyContent: "center" }}>
      <div style={{ ...DISPLAY, fontSize: size, color: GROUND, marginLeft: layout.margin }}>
        <div
          style={{
            height: `${String(0.98 * (1 - gone))}em`,
            opacity: 1 - faded,
            transform: `translateY(${String(-gone * 0.3)}em)`,
            color: RED,
          }}
        >
          <div style={{ position: "relative", display: "inline-block" }}>
            <Rise text="Um, uh," start={0} />
            <span
              style={{
                position: "absolute",
                left: 0,
                top: "0.5em",
                height: "0.07em",
                width: `${String(strike * 100)}%`,
                background: GROUND,
              }}
            />
          </div>
        </div>
        <div style={{ display: "flex", whiteSpace: "pre" }}>
          <Swap from="n" to="N" enter={4} at={34} frame={frame} />
          <Rise text={`o ${rest[0] ?? ""}`} start={4} style={{ display: "inline-block" }} />
        </div>
        {rest.slice(1).map((line) => (
          <Rise key={line} text={line} start={8} />
        ))}
      </div>
    </AbsoluteFill>
  );
}

/** Beat 5b, on red: the capital and the full stop arrive. */
export function Punctuation({ layout }: { layout: Layout }) {
  const frame = useCurrentFrame();
  const stop = ramp(frame, 30, 8, resolve);
  return (
    <AbsoluteFill style={{ background: RED, justifyContent: "center" }}>
      <div style={{ ...DISPLAY, fontSize: layout.benefit, color: GROUND, marginLeft: layout.margin }}>
        <div style={{ display: "flex" }}>
          <Swap from="p" to="P" enter={0} at={22} frame={frame} />
          <Rise text="roper" start={0} style={{ display: "inline-block" }} />
        </div>
        <div style={{ display: "flex", alignItems: "baseline" }}>
          <Rise text="punctuation" start={4} style={{ display: "inline-block" }} />
          <span
            style={{
              display: "inline-block",
              color: INK,
              opacity: stop,
              transform: `translateY(${String((1 - stop) * -0.5)}em) scale(${String(0.4 + 0.6 * stop)})`,
              transformOrigin: "50% 80%",
            }}
          >
            .
          </span>
        </div>
      </div>
    </AbsoluteFill>
  );
}
