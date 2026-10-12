import { Composition } from "remotion";
import { DURATION, FPS } from "./timeline";
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
    </>
  );
}
