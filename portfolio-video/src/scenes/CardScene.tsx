/**
 * The portrait card for the portfolio's filmstrip carousel: 3:4, the
 * dictation recomposed for portrait rather than cropped from 16:9.
 *
 * In the carousel an unfocused card shows only its top half, positioned at
 * 50% 26% — on a 1200 × 1600 card, the band from y 208 to y 1008. So the
 * sentence Klar typed and the pill that typed it both sit in that band: a
 * tall email window fills the card, and the overlay floats over it just under
 * the sentence, as it floats over whatever application is in front.
 */

import "../../../src/styles/global.css";
import "../../../src/App.css";
import "../video.css";
import "../fonts";

import { AbsoluteFill, useVideoConfig } from "remotion";
import type { ComposeLayout } from "../format";
import { CARD_CUES } from "../timeline";
import { GhostLine } from "../ui/GhostLine";
import { MailWindow } from "../ui/MailWindow";
import { Pill } from "../ui/Pill";
import { Theme } from "../ui/Theme";

/**
 * Stage pixels: 450 × 600, drawn at 2⅔× for the 1200 × 1600 card, where the
 * carousel's band is stage y 78 to 378. In it: the To and Subject rows, the
 * sentence (y 182–246) and the pill (y 280–344).
 */
const CARD: ComposeLayout = {
  width: 450,
  height: 600,
  mail: { x: 22, y: 36, width: 406, height: 524 },
  ghost: { top: 214, width: 380 },
  pillTop: 280,
};

export function CardScene() {
  const { width } = useVideoConfig();

  return (
    <AbsoluteFill className="kv" style={{ background: "var(--kv-ground)" }}>
      <Theme dark={false} />
      <div
        className="kv-stage"
        style={{ width: CARD.width, height: CARD.height, transform: `scale(${String(width / CARD.width)})` }}
      >
        <MailWindow box={CARD.mail} enter={1} cues={CARD_CUES} className="kv-mail--card" />
        <GhostLine layout={CARD} cues={CARD_CUES} />
        <Pill layout={CARD} cues={CARD_CUES} />
      </div>
    </AbsoluteFill>
  );
}
