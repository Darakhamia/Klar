import type { CSSProperties } from "react";
import { useCurrentFrame } from "remotion";
import type { Layout } from "../format";
import { ramp, resolve } from "../motion";
import { T } from "../timeline";

/**
 * The end card, which is also the first second of the video: the loop joins
 * inside its still hold, so the last frame and the first are the same picture.
 */
export function EndCard({ layout }: { layout: Layout }) {
  const frame = useCurrentFrame();
  const arriving = frame >= T.endIn;
  if (!arriving && frame >= T.composeIn) return null;

  // Arriving: each part rises in after the one before. Leaving, at the seam:
  // everything goes together, drifting up a little.
  const shown = (after: number): number =>
    arriving ? ramp(frame, T.endIn + after, 14, resolve) : 1 - ramp(frame, T.seamOut, 12, resolve);
  const rise = (after: number): CSSProperties => {
    const p = shown(after);
    return {
      opacity: p,
      transform: `translateY(${String(arriving ? 12 * (1 - p) : -6 * (1 - p))}px)`,
    };
  };

  return (
    <div className="kv-end" style={{ paddingLeft: layout.margin }}>
      <div className="kv-end__mark" style={{ fontSize: layout.wordmark, ...rise(0) }}>
        Klar
      </div>
      <div
        className="kv-end__rule"
        style={{ width: 56 * (arriving ? shown(6) : 1), opacity: arriving ? 1 : shown(0) }}
      />
      <div className="kv-end__line" style={rise(6)}>
        Speak. Get clean text.
      </div>
      <div className="kv-end__url" style={rise(12)}>
        getklar.net
      </div>
    </div>
  );
}
