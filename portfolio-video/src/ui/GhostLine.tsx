import { Fragment } from "react";
import { useCurrentFrame } from "remotion";
import type { ComposeLayout } from "../format";
import { ramp, resolve } from "../motion";
import { SPOKEN, spokenAt } from "../speech";
import { VIDEO_CUES, type Cues } from "../timeline";

/**
 * What the speaker actually said, faint, word by word as it is said. Once the
 * key comes up the fillers go first, then the line sinks into the pill.
 */
export function GhostLine({ layout, cues = VIDEO_CUES }: { layout: ComposeLayout; cues?: Cues }) {
  const frame = useCurrentFrame();
  if (frame < cues.pillIn || frame > cues.release + 24) return null;

  const sink = ramp(frame, cues.release + 6, 14, resolve);

  return (
    <div
      className="kv-ghost"
      style={{
        left: layout.mail.x + (layout.mail.width - layout.ghost.width) / 2,
        width: layout.ghost.width,
        top: layout.ghost.top,
        opacity: 1 - sink,
        transform: `translateY(${String(14 * sink)}px)`,
      }}
    >
      {SPOKEN.map((word, index) => {
        const heard = ramp(frame, spokenAt(word, cues) + 2, 5, resolve);
        const dropped = word.filler ? ramp(frame, cues.release, 8) : 0;
        return (
          <Fragment key={index}>
            <span
              className="kv-ghost__word"
              style={{
                opacity: heard * (1 - dropped),
                transform: `translateY(${String(4 * (1 - heard))}px)`,
              }}
            >
              {word.text}
            </span>{" "}
          </Fragment>
        );
      })}
    </div>
  );
}
