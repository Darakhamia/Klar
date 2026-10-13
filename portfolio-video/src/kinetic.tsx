/**
 * Kinetic type: words rising out of a mask, one after another, on the app's
 * own curve. Every line in the video is set through this.
 */

import { Fragment, type CSSProperties } from "react";
import { useCurrentFrame } from "remotion";
import { ramp, resolve } from "./motion";

/** One line of words, each rising into place from just below its mask. */
export function Rise({
  text,
  start,
  stagger = 3,
  frames = 16,
  style,
}: {
  text: string;
  /** Frame (within the scene) the first word starts to rise. */
  start: number;
  stagger?: number;
  frames?: number;
  style?: CSSProperties;
}) {
  const frame = useCurrentFrame();
  return (
    <div style={{ whiteSpace: "nowrap", ...style }}>
      {text.split(" ").map((word, index) => {
        const t = ramp(frame, start + index * stagger, frames, resolve);
        return (
          <Fragment key={index}>
            {index > 0 && " "}
            <span
              style={{
                display: "inline-block",
                overflow: "hidden",
                verticalAlign: "top",
                // Room for descenders inside the mask.
                padding: "0.06em 0 0.14em",
                margin: "-0.06em 0 -0.14em",
              }}
            >
              <span style={{ display: "inline-block", transform: `translateY(${String((1 - t) * 110)}%)` }}>
                {word}
              </span>
            </span>
          </Fragment>
        );
      })}
    </div>
  );
}

/** 1 → 0 across `frames` from `start`: for whatever leaves at the end of a beat. */
export const leaving = (frame: number, start: number, frames: number) => 1 - ramp(frame, start, frames, resolve);
