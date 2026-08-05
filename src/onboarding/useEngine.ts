/**
 * What onboarding needs from the engine.
 *
 * The same single event channel the overlay reads, reduced to the four things
 * these three screens care about: the input level, whether the hotkey is live,
 * the text of a finished dictation, and anything that went wrong.
 */

import { useEffect, useState } from "react";
import {
  ENGINE_EVENT,
  subscribe,
  type Binding,
  type EngineEvent,
  type PipelineState,
} from "../lib/ipc";

/** Bars in the level meter, from the design's first screen. */
export const BARS = 8;

/** Above this peak we are hearing a voice rather than a room. */
const HEARD = 0.02;

export interface EngineView {
  state: PipelineState;
  /** Newest last, 0..1. */
  levels: number[];
  /** Set the first time a level arrives that is loud enough to be speech, and
   * left set: a pause between words must not read as the microphone failing. */
  heard: boolean;
  /** The last dictation that finished, or null if there has not been one. */
  said: string | null;
  /** Set once the models are loaded and the hotkey is registered. */
  hotkey: Binding | null;
  error: string | null;
}

const QUIET: number[] = Array.from({ length: BARS }, () => 0);

export function useEngine(): EngineView {
  const [state, setState] = useState<PipelineState>("idle");
  const [levels, setLevels] = useState<number[]>(QUIET);
  const [heard, setHeard] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const [hotkey, setHotkey] = useState<Binding | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    return subscribe<EngineEvent>(
      ENGINE_EVENT,
      (payload) => {
        switch (payload.kind) {
          case "state":
            setState(payload.state);
            if (payload.state === "recording") {
              setSaid(null);
              setError(null);
            }
            break;
          case "level":
            setLevels((previous) => [...previous.slice(1), payload.peak]);
            if (payload.peak > HEARD) setHeard(true);
            break;
          case "text":
            // Only a settled result counts: the partials exist to make the
            // overlay feel alive, and showing one here as "what you said" would
            // put a sentence on screen that is still going to change.
            if (payload.settled) setSaid(payload.text);
            break;
          case "failed":
            setError(payload.message);
            break;
          case "ready":
            setHotkey(payload.hotkey);
            setError(null);
            break;
          case "loading":
            break;
        }
      },
      setError,
    );
  }, []);

  return { state, levels, heard, said, hotkey, error };
}
