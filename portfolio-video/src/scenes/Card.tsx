import type { ReactNode } from "react";
import { useCurrentFrame } from "remotion";
import { ramp, resolve } from "../motion";

/**
 * How a capture floats: rises in, tilted back a few degrees, and settles flat
 * while the camera pushes in slowly for the rest of the beat.
 */
export function Float({
  children,
  enterAt = 0,
  duration,
  tilt = 10,
  push = 0.04,
}: {
  children: ReactNode;
  enterAt?: number;
  duration: number;
  tilt?: number;
  push?: number;
}) {
  const frame = useCurrentFrame();
  // On a cut, the capture is already arriving on the beat's first frame.
  const shown = ramp(frame, enterAt - 3, 8, resolve);
  const enter = ramp(frame, enterAt - 3, 16, resolve);
  const settle = ramp(frame, enterAt - 3, 40, resolve);
  // Starts a touch small and grows to exactly 1 on the last frame, so the
  // capture is never drawn above its own resolution.
  const scale = 1 - push + push * ramp(frame, enterAt, duration - enterAt);
  return (
    <div style={{ perspective: 2400 }}>
      <div
        style={{
          opacity: shown,
          // The tilt is dropped once it has settled, so a resting capture is
          // drawn flat rather than through a 3D layer.
          transform: `translateY(${String((1 - enter) * 60)}px)${settle < 1 ? ` rotateX(${String((1 - settle) * tilt)}deg)` : ""} scale(${String(scale)})`,
          transformOrigin: "50% 60%",
          filter:
            "drop-shadow(0 40px 60px rgb(32 30 29 / 0.22)) drop-shadow(0 6px 14px rgb(32 30 29 / 0.12))",
        }}
      >
        {children}
      </div>
    </div>
  );
}
