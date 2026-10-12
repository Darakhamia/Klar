import { useCurrentFrame } from "remotion";
import type { Layout } from "../format";
import { ramp, resolve } from "../motion";
import { CUES } from "../timeline";

/** Bottom-left, on the shared margin, arriving with the action they name. */
export function Captions({ layout }: { layout: Layout }) {
  const frame = useCurrentFrame();

  return (
    <>
      {CUES.map((cue) => {
        if (frame < cue.from || frame > cue.to + 8) return null;
        const enter = ramp(frame, cue.from, 12, resolve);
        const leave = ramp(frame, cue.to, 8);
        return (
          <div
            key={cue.text}
            className="kv-caption"
            style={{
              left: layout.margin,
              bottom: layout.captionBottom,
              color: cue.tone === "dark" ? "var(--color-neutral-100)" : "var(--kv-ink)",
              opacity: enter * (1 - leave),
              transform: `translateY(${String(12 * (1 - enter))}px)`,
            }}
          >
            {cue.text}
          </div>
        );
      })}
    </>
  );
}
