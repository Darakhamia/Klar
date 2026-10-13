import { AbsoluteFill, useCurrentFrame } from "remotion";
import { Crop, around, useManifest, within, type Box } from "../captures";
import { Rise } from "../kinetic";
import type { Format, Layout } from "../layout";
import { ramp, resolve } from "../motion";
import { DISPLAY, GROUND, INK, RED_DARK_UI } from "../theme";
import { Float } from "./Card";

const HEAD: Record<Format, string[]> = {
  wide: ["Your own vocabulary."],
  square: ["Your own", "vocabulary."],
};

/**
 * Beat 5c: the real Dictionary screen, cropped to its live preview. What
 * whisper heard is struck through in the sentence typed in; the term the
 * dictionary put in its place is underlined in the preview below it.
 */
export function Vocabulary({ layout, format, duration }: { layout: Layout; format: Format; duration: number }) {
  const frame = useCurrentFrame();
  const { dictionary } = useManifest();
  const b = dictionary.boxes;
  const scale = layout.vocabulary;
  const room = layout.width - 2 * layout.margin;
  // The try field and the preview, cut on the right where the text has ended.
  const full = around([b.tryField, b.preview], 18);
  const crop: Box = { ...full, width: Math.min(full.width, room / scale) };
  const rule = 2 * scale;
  const strike = ramp(frame, 30, 10, resolve);
  const underline = ramp(frame, 44, 12, resolve);
  const heard = within(b.heard, crop, scale);
  // The card hangs a clear gap below the headline.
  const cardTop = layout.height * 0.1 + HEAD[format].length * layout.head + layout.head * 0.7;
  const term = within(b.term, crop, scale);
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
        <Float enterAt={6} duration={duration} tilt={10} push={0.04}>
          <Crop page={dictionary} crop={crop} scale={scale}>
            <span
              style={{
                position: "absolute",
                left: heard.x,
                top: heard.y + heard.height * 0.42,
                width: heard.width * strike,
                height: rule,
                background: RED_DARK_UI,
              }}
            />
            <span
              style={{
                position: "absolute",
                left: term.x,
                top: term.y + term.height + rule,
                width: term.width * underline,
                height: rule * 1.5,
                background: RED_DARK_UI,
              }}
            />
          </Crop>
        </Float>
      </div>
    </AbsoluteFill>
  );
}
