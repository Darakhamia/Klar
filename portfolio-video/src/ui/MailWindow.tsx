import { Fragment } from "react";
import { useCurrentFrame } from "remotion";
import { MAIL } from "../demo";
import type { Box } from "../format";
import { ramp } from "../motion";
import { CLEAN, VIDEO_CUES, type Cues } from "../timeline";
import { Controls } from "./Controls";

const WORDS = CLEAN.split(" ");
/** A text caret's blink, about half a second each way. */
const BLINK = 16;

/** A plain email compose window — the application being dictated into. */
export function MailWindow({
  box,
  enter,
  cues = VIDEO_CUES,
  className,
}: {
  box: Box;
  enter: number;
  cues?: Cues;
  className?: string;
}) {
  const frame = useCurrentFrame();

  // The sentence lands left to right, word by word, across `landFrames`.
  const step = cues.landFrames / WORDS.length;
  const cleared = cues.clear !== null && frame >= cues.clear + 10;
  const landed = cleared ? 0 : WORDS.filter((_, index) => frame >= cues.land + index * step).length;
  const gone = cues.clear === null ? 0 : ramp(frame, cues.clear, 10);
  const typing = frame >= cues.land && frame < cues.land + cues.landFrames + BLINK;
  const lit =
    cues.blinkFrom === null || typing || Math.floor((frame - cues.blinkFrom) / BLINK) % 2 === 0;
  const caret = <span className="kv-caret" style={{ opacity: lit ? 1 : 0 }} />;

  return (
    <div
      className={className === undefined ? "kv-mail" : `kv-mail ${className}`}
      style={{
        left: box.x,
        top: box.y,
        width: box.width,
        height: box.height,
        opacity: enter,
        transform: `translateY(${String(16 * (1 - enter))}px)`,
      }}
    >
      <div className="kv-mail__bar">
        <span>New message</span>
        <Controls />
      </div>
      <div className="kv-mail__field">
        <span className="kv-mail__key">To</span>
        <span>{MAIL.to}</span>
      </div>
      <div className="kv-mail__field">
        <span className="kv-mail__key">Subject</span>
        <span>{MAIL.subject}</span>
      </div>
      <div className="kv-mail__body">
        {landed === 0 && caret}
        {WORDS.map((word, index) => (
          <Fragment key={index}>
            <span style={{ opacity: ramp(frame, cues.land + index * step, 4) * (1 - gone) }}>{word}</span>
            {index === landed - 1 && caret}{" "}
          </Fragment>
        ))}
      </div>
      <div className="kv-mail__foot">
        <span className="kv-mail__send">Send</span>
        <span className="kv-mail__discard">Discard</span>
      </div>
    </div>
  );
}
