import { Fragment } from "react";
import { AbsoluteFill, useCurrentFrame } from "remotion";
import copy from "../../copy.json";
import type { Layout } from "../layout";
import { ramp, resolve } from "../motion";
import { FONT, GROUND, INK, MUTED, WHITE } from "../theme";

const WORDS = copy.clean.split(" ");
const LAND = 14; // the first word, from the start of the beat
const LAND_FRAMES = 24; // 0.8 s for the whole sentence
const BLINK = 16;

/**
 * Beat 4: the clean sentence lands in an email — another app, drawn for the
 * video, in close-up. Words arrive left to right in 0.8 s, tinted the way a
 * fresh insert is, and the tint fades once they have all landed.
 */
export function Email({ layout, duration }: { layout: Layout; duration: number }) {
  const frame = useCurrentFrame();
  const e = layout.email;
  const enter = ramp(frame, -3, 14, resolve);
  const shown = ramp(frame, -3, 8, resolve);
  const push = 1 + 0.035 * ramp(frame, 0, duration);
  const step = LAND_FRAMES / WORDS.length;
  // A word counts as landed once it is mostly there; the caret follows it.
  const landed = WORDS.filter((_, index) => frame >= LAND + index * step + 2).length;
  const tint = 1 - ramp(frame, LAND + LAND_FRAMES + 22, 16);
  const typing = frame >= LAND && frame < LAND + LAND_FRAMES + BLINK;
  // Blinks while it waits for the text, holds still while text arrives.
  const caretOn = typing || Math.floor(Math.abs(frame - LAND - LAND_FRAMES) / BLINK) % 2 === 0;
  const caret = (
    <span
      style={{
        display: "inline-block",
        width: "0.045em",
        height: "1em",
        marginLeft: "0.04em",
        marginRight: "-0.085em",
        verticalAlign: "-0.14em",
        background: INK,
        opacity: caretOn ? 1 : 0,
      }}
    />
  );
  const rule = `${String(Math.round(e.meta / 20))}px solid #e6e3e3`;
  const pad = e.meta * 1.6;

  return (
    <AbsoluteFill style={{ background: GROUND }}>
      <div
        style={{
          position: "absolute",
          left: (layout.width - e.width) / 2,
          top: e.top,
          width: e.width,
          height: layout.height,
          background: WHITE,
          fontFamily: FONT,
          color: INK,
          opacity: shown,
          transform: `translateY(${String((1 - enter) * 80)}px) scale(${String(push)})`,
          transformOrigin: "30% 40%",
          boxShadow: "0 40px 90px rgb(32 30 29 / 0.16), 0 6px 16px rgb(32 30 29 / 0.08)",
        }}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            padding: `${String(e.meta * 0.8)}px ${String(pad)}px`,
            borderBottom: rule,
            fontSize: e.meta,
            fontWeight: 600,
          }}
        >
          <span>New message</span>
          <span style={{ display: "flex", gap: e.meta * 0.4 }}>
            {[0, 1, 2].map((dot) => (
              <span key={dot} style={{ width: e.meta * 0.36, height: e.meta * 0.36, background: "#d7d3d3" }} />
            ))}
          </span>
        </div>
        {[
          ["To", copy.mail.to],
          ["Subject", copy.mail.subject],
        ].map(([key, value]) => (
          <div
            key={key}
            style={{
              display: "flex",
              gap: e.meta,
              padding: `${String(e.meta * 0.55)}px ${String(pad)}px`,
              borderBottom: rule,
              fontSize: e.meta,
            }}
          >
            <span style={{ color: MUTED, width: e.meta * 4 }}>{key}</span>
            <span>{value}</span>
          </div>
        ))}
        <div style={{ padding: `${String(e.text * 0.7)}px ${String(pad)}px`, fontSize: e.text, lineHeight: 1.32 }}>
          {landed === 0 && caret}
          {/* Landed words share one run of --color-accent-200, the tint of a
              fresh insert, so the tint has no seams between words. */}
          <span style={{ background: `rgb(255 224 217 / ${String(tint)})` }}>
            {WORDS.slice(0, landed).map((word, index) => (
              <Fragment key={index}>
                {index > 0 && " "}
                <span style={{ opacity: ramp(frame, LAND + index * step, 4, resolve) }}>{word}</span>
              </Fragment>
            ))}
          </span>
          {landed > 0 && caret}
          {/* The rest hold their place, unseen, so lines never reflow. */}
          <span style={{ opacity: 0 }}>
            {WORDS.slice(landed).map((word, index) => (
              <Fragment key={index}>
                {(landed > 0 || index > 0) && " "}
                {word}
              </Fragment>
            ))}
          </span>
        </div>
      </div>
    </AbsoluteFill>
  );
}

