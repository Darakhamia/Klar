import { AbsoluteFill, useCurrentFrame } from "remotion";
import type { Layout } from "../layout";
import { ramp, resolve } from "../motion";
import { DISPLAY, FONT, GROUND, INK, RED, WHITE } from "../theme";

/** Beat 3a: the shortcut, two keycaps, dropped in and then held down. */
export function Keys({ layout, pressAt }: { layout: Layout; pressAt: number }) {
  const frame = useCurrentFrame();
  const k = layout.keys;
  const pressed = ramp(frame, pressAt, 3);
  return (
    <AbsoluteFill style={{ background: GROUND, alignItems: "center", justifyContent: "center" }}>
      <div style={{ display: "flex", alignItems: "center", gap: k.gap }}>
        <Keycap label="Ctrl" width={k.ctrl} layout={layout} enter={ramp(frame, -4, 12, resolve)} pressed={pressed} />
        <div
          style={{
            ...DISPLAY,
            fontSize: k.plus,
            color: RED,
            opacity: ramp(frame, 0, 10, resolve),
            transform: `translateY(${String(-k.height * 0.05)}px)`,
          }}
        >
          +
        </div>
        <Keycap
          label="Space"
          width={k.space}
          layout={layout}
          enter={ramp(frame, -1, 12, resolve)}
          pressed={pressed}
        />
      </div>
    </AbsoluteFill>
  );
}

function Keycap({
  label,
  width,
  layout,
  enter,
  pressed,
}: {
  label: string;
  width: number;
  layout: Layout;
  enter: number;
  pressed: number;
}) {
  const k = layout.keys;
  // The key's travel: its ink side shows `depth` below the face until pressed.
  const depth = k.height * 0.09;
  const showing = depth - (depth - k.height * 0.015) * pressed;
  const rule = Math.round(k.height / 60);
  return (
    <div
      style={{
        position: "relative",
        width,
        height: k.height,
        opacity: enter,
        transform: `translateY(${String((1 - enter) * -k.height * 0.35 + (depth - showing))}px)`,
      }}
    >
      <div
        style={{
          position: "absolute",
          inset: 0,
          background: WHITE,
          border: `${String(rule)}px solid ${INK}`,
          boxShadow: `0 ${String(showing)}px 0 0 ${INK}, 0 ${String(showing + k.height * 0.16)}px ${String(k.height * 0.3)}px rgb(32 30 29 / ${String(0.2 - 0.08 * pressed)})`,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontFamily: FONT,
          fontWeight: 700,
          fontSize: k.label,
          letterSpacing: "0.14em",
          textTransform: "uppercase",
          color: INK,
          paddingLeft: "0.14em",
        }}
      >
        {label}
      </div>
    </div>
  );
}
