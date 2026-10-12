import { useCurrentFrame } from "remotion";
import type { Layout } from "../format";
import { aim, ramp, resolve } from "../motion";
import { T } from "../timeline";
import { Camera } from "../ui/Camera";
import { GhostLine } from "../ui/GhostLine";
import { MailWindow } from "../ui/MailWindow";
import { Pill } from "../ui/Pill";

/** Scenes 1–3: an email, dictated. */
export function ComposeScene({ layout }: { layout: Layout }) {
  const frame = useCurrentFrame();

  return (
    <div className="kv-layer">
      <Camera layout={layout} shot={aim(frame, layout.composeShots)}>
        <MailWindow box={layout.mail} enter={ramp(frame, T.composeIn, 21, resolve)} />
        <GhostLine layout={layout} />
        <Pill layout={layout} />
      </Camera>
    </div>
  );
}
