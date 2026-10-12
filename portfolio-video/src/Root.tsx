import { Composition } from "remotion";
import { CardScene } from "./scenes/CardScene";
import { CARD_DURATION, DURATION, FPS } from "./timeline";
import { Video } from "./Video";

export function Root() {
  return (
    <>
      <Composition
        id="Klar16x9"
        component={Video}
        durationInFrames={DURATION}
        fps={FPS}
        width={1920}
        height={1080}
        defaultProps={{ format: "wide" as const }}
      />
      <Composition
        id="Klar1x1"
        component={Video}
        durationInFrames={DURATION}
        fps={FPS}
        width={1080}
        height={1080}
        defaultProps={{ format: "square" as const }}
      />
      {/* The portfolio's portrait card. The still renders at this size; the
          loop renders at 0.75, 900 × 1200. */}
      <Composition
        id="KlarCard"
        component={CardScene}
        durationInFrames={CARD_DURATION}
        fps={FPS}
        width={1200}
        height={1600}
      />
    </>
  );
}
