import { Fragment } from "react";
import { useCurrentFrame } from "remotion";
import { MAIL } from "../demo";
import type { Box } from "../format";
import { ramp } from "../motion";
import { CLEAN, T } from "../timeline";
import { Controls } from "./Controls";

const WORDS = CLEAN.split(" ");
/** The sentence lands left to right: one word every other frame. */
const STEP = T.landFrames / WORDS.length;
/** A text caret's blink, about half a second each way. */
const BLINK = 16;

/** A plain email compose window — the application being dictated into. */
export function MailWindow({ box, enter }: { box: Box; enter: number }) {
  const frame = useCurrentFrame();

  const landed = WORDS.filter((_, index) => frame >= T.land + index * STEP).length;
  const typing = frame >= T.land && frame < T.land + T.landFrames + BLINK;
  const lit = typing || Math.floor((frame - T.composeIn) / BLINK) % 2 === 0;
  const caret = <span className="kv-caret" style={{ opacity: lit ? 1 : 0 }} />;

  return (
    <div
      className="kv-mail"
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
            <span style={{ opacity: ramp(frame, T.land + index * STEP, 4) }}>{word}</span>
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
