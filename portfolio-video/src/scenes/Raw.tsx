import { AbsoluteFill, useCurrentFrame } from "remotion";
import copy from "../../copy.json";
import type { Format, Layout } from "../layout";
import { ramp, resolve } from "../motion";
import { FONT, GROUND, INK, RED } from "../theme";

const WORDS = copy.raw.split(" ");
/** Which words are filler: "so", "um", "uh", and the repeated "can you". */
const FILLER = new Set([1, 2, 8, 11, 12]);
/** When each word arrives, in frames from the start of the beat: speech, not a metronome. */
const ARRIVES = [2, 8, 12, 18, 23, 29, 33, 39, 46, 52, 56, 60, 64, 70, 75, 79, 84, 88];
const WEIGHT = 500;
const LEADING = 1.12;
/** How many lines show at once; older lines scroll up and out. */
const ROWS: Record<Format, number> = { wide: 3, square: 4 };

/** Word widths in Archivo, measured the way the browser will set them. */
function measure(size: number) {
  const context = document.createElement("canvas").getContext("2d");
  if (context === null) throw new Error("no 2d canvas");
  context.font = `${String(WEIGHT)} ${String(size)}px Archivo`;
  return { space: context.measureText(" ").width, widths: WORDS.map((word) => context.measureText(word).width) };
}

interface Placed {
  line: number;
  x: number;
  width: number;
}

/**
 * Beat 2: what you actually say, arriving word by word as speech does, in big
 * lines that scroll up as they fill. The filler words are red.
 */
export function Raw({ layout, format }: { layout: Layout; format: Format }) {
  const frame = useCurrentFrame();
  const size = layout.raw;
  const lh = size * LEADING;
  const rows = ROWS[format];
  const { space, widths } = measure(size);

  // Break into lines the width of the frame's text column.
  const column = layout.width - 2 * layout.margin;
  const placed: Placed[] = [];
  let line = 0;
  let x = 0;
  widths.forEach((width) => {
    if (x > 0 && x + width > column) {
      line++;
      x = 0;
    }
    placed.push({ line, x, width });
    x += width + space;
  });

  // Once a line past the last row starts, everything moves up a line.
  let scroll = 0;
  placed.forEach((word, index) => {
    const startsLine = index === 0 || placed[index - 1]?.line !== word.line;
    if (startsLine && word.line >= rows) scroll += ramp(frame, ARRIVES[index] ?? 0, 10, resolve);
  });

  // The caret follows the newest word as it arrives.
  let caret = { x: -space * 0.6, line: 0 };
  placed.forEach((word, index) => {
    const t = ramp(frame, ARRIVES[index] ?? 0, 6, resolve);
    if (t <= 0) return;
    const from = caret.line === word.line ? caret.x : word.x - space * 0.6;
    caret = { x: from + (word.x + word.width + space * 0.4 - from) * t, line: word.line };
  });

  const top = (layout.height - rows * lh) / 2;
  const y = (lineIndex: number) => top + (lineIndex - scroll) * lh;
  // Lines fade as they leave the top row.
  const fade = (lineIndex: number) => Math.max(0, Math.min(1, 1 - (top - y(lineIndex)) / lh));

  return (
    <AbsoluteFill style={{ background: GROUND }}>
      <div
        style={{
          position: "absolute",
          inset: 0,
          fontFamily: FONT,
          fontWeight: WEIGHT,
          fontSize: size,
          lineHeight: `${String(lh)}px`,
          whiteSpace: "nowrap",
          color: INK,
        }}
      >
        {WORDS.map((word, index) => {
          const at = placed[index];
          if (at === undefined) return null;
          const t = ramp(frame, ARRIVES[index] ?? Number.POSITIVE_INFINITY, 6, resolve);
          return (
            <span
              key={index}
              style={{
                position: "absolute",
                left: layout.margin + at.x,
                top: y(at.line),
                color: FILLER.has(index) ? RED : INK,
                opacity: t * fade(at.line),
                transform: `translateY(${String((1 - t) * 0.25)}em)`,
              }}
            >
              {word}
            </span>
          );
        })}
        <span
          style={{
            position: "absolute",
            left: layout.margin + caret.x,
            top: y(caret.line) + (lh - size) / 2 + size * 0.1,
            width: size * 0.06,
            height: size,
            background: RED,
            opacity: fade(caret.line),
          }}
        />
      </div>
    </AbsoluteFill>
  );
}
