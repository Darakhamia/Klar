import { Composition } from "remotion";
import { Promo } from "./Promo";
import { DURATION, FPS } from "./timeline";

export function Root() {
  return (
    <>
      <Composition
        id="Klar16x9"
        component={Promo}
        durationInFrames={DURATION}
        fps={FPS}
        width={1920}
        height={1080}
        defaultProps={{ format: "wide" as const }}
      />
      <Composition
        id="Klar1x1"
        component={Promo}
        durationInFrames={DURATION}
        fps={FPS}
        width={1080}
        height={1080}
        defaultProps={{ format: "square" as const }}
      />
    </>
  );
}
