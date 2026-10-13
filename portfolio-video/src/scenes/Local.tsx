import { AbsoluteFill, useCurrentFrame } from "remotion";
import { Crop, around, useManifest, within } from "../captures";
import { Rise } from "../kinetic";
import type { Format, Layout } from "../layout";
import { ramp, resolve } from "../motion";
import { DISPLAY, GROUND, INK, RED_DARK_UI } from "../theme";
import { Float } from "./Card";

const HEAD: Record<Format, string[]> = {
  wide: ["Runs on your computer.", "Nothing leaves it."],
  square: ["Runs on your", "computer.", "Nothing leaves it."],
};

/**
 * Beat 6: the real Voice panel's Processing row, with the sentence that says
 * where speech goes — nowhere — underlined line by line.
 */
export function Local({ layout, format, duration }: { layout: Layout; format: Format; duration: number }) {
  const frame = useCurrentFrame();
  const { voice } = useManifest();
  const b = voice.boxes;
  const scale = layout.local;
  // Stops short of the row's own divider underneath.
  const crop = around([b.label, b.hint], 14);
  const rule = 2 * scale;
  const lines = b.sentence.map((box) => within(box, crop, scale));
  const total = lines.reduce((sum, line) => sum + line.width, 0);
  // One sweep across both lines, at an even speed.
  const swept = ramp(frame, 40, 22, resolve) * total;
  // How much of the sweep each line has had by the time it starts.
  const starts = lines.map((_, index) => lines.slice(0, index).reduce((sum, line) => sum + line.width, 0));
  // The card hangs a clear gap below the headline.
  const cardTop = layout.height * 0.1 + HEAD[format].length * layout.head + layout.head * 0.7;
  return (
    <AbsoluteFill style={{ background: GROUND }}>
      <div
        style={{
          ...DISPLAY,
          position: "absolute",
          left: layout.margin,
          top: layout.height * 0.1,
          fontSize: layout.head,
          color: INK,
        }}
      >
        {HEAD[format].map((line, index) => (
          <Rise key={line} text={line} start={index * 5} />
        ))}
      </div>
      <div style={{ position: "absolute", left: layout.margin, top: cardTop }}>
        <Float enterAt={8} duration={duration} tilt={10} push={0.04}>
          <Crop page={voice} crop={crop} scale={scale}>
            {lines.map((line, index) => {
              const width = Math.max(0, Math.min(line.width, swept - (starts[index] ?? 0)));
              return (
                <span
                  key={index}
                  style={{
                    position: "absolute",
                    left: line.x,
                    top: line.y + line.height + rule * 0.5,
                    width,
                    height: rule * 1.5,
                    background: RED_DARK_UI,
                  }}
                />
              );
            })}
          </Crop>
        </Float>
      </div>
    </AbsoluteFill>
  );
}
