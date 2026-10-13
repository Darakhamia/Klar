import { useEffect, useState } from "react";
import { AbsoluteFill, Sequence, cancelRender, continueRender, delayRender, staticFile } from "remotion";
import { ManifestContext, type Manifest } from "./captures";
import { LAYOUTS, type Format } from "./layout";
import { Filler, Punctuation } from "./scenes/Benefits";
import { Email } from "./scenes/Email";
import { End } from "./scenes/End";
import { Hook } from "./scenes/Hook";
import { Keys } from "./scenes/Keys";
import { Local } from "./scenes/Local";
import { Pill } from "./scenes/Pill";
import { Raw } from "./scenes/Raw";
import { Vocabulary } from "./scenes/Vocabulary";
import { T } from "./timeline";

const length = (beat: { from: number; to: number }) => beat.to - beat.from;

/** The whole promo, in either format. Beats and timing: timeline.ts. */
export function Promo({ format }: { format: Format }) {
  const layout = LAYOUTS[format];
  const [manifest, setManifest] = useState<Manifest | null>(null);
  const [handle] = useState(() => delayRender("Reading captures/manifest.json"));

  useEffect(() => {
    fetch(staticFile("captures/manifest.json"))
      .then((response) => {
        if (!response.ok) throw new Error("No captures: run `npm run capture` first.");
        return response.json() as Promise<Manifest>;
      })
      .then((loaded) => {
        setManifest(loaded);
        continueRender(handle);
      })
      .catch((error: unknown) => {
        cancelRender(error);
      });
  }, [handle]);

  if (manifest === null) return null;

  const at = (beat: { from: number; to: number }) => ({ from: beat.from, durationInFrames: length(beat) });
  // The red curtain of beat 1 lifts over the start of beat 2.
  const curtain = 14;

  return (
    <ManifestContext value={manifest}>
      <AbsoluteFill>
        <Sequence {...at(T.raw)}>
          <Raw layout={layout} format={format} />
        </Sequence>
        <Sequence from={T.hook.from} durationInFrames={length(T.hook) + curtain}>
          <Hook layout={layout} format={format} liftAt={length(T.hook)} />
        </Sequence>
        <Sequence {...at(T.keys)}>
          <Keys layout={layout} pressAt={22} />
        </Sequence>
        <Sequence {...at(T.pill)}>
          <Pill layout={layout} duration={length(T.pill)} />
        </Sequence>
        <Sequence {...at(T.email)}>
          <Email layout={layout} duration={length(T.email)} />
        </Sequence>
        <Sequence {...at(T.filler)}>
          <Filler layout={layout} format={format} />
        </Sequence>
        <Sequence {...at(T.punctuation)}>
          <Punctuation layout={layout} />
        </Sequence>
        <Sequence {...at(T.vocabulary)}>
          <Vocabulary layout={layout} format={format} duration={length(T.vocabulary)} />
        </Sequence>
        <Sequence {...at(T.local)}>
          <Local layout={layout} format={format} duration={length(T.local)} />
        </Sequence>
        <Sequence {...at(T.end)}>
          <End layout={layout} clearAt={length(T.end) - 12} />
        </Sequence>
      </AbsoluteFill>
    </ManifestContext>
  );
}
