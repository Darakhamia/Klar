/**
 * The overlay's whole connection to the app.
 *
 * It subscribes to one event channel and holds no state the engine does not
 * give it. Every visual state below comes from a transition the Rust side
 * decided; nothing here infers what the pipeline is doing.
 */

import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ENGINE_EVENT, type EngineEvent, type PipelineState } from "../lib/ipc";

export type { PipelineState };

/** How many level readings the waveform shows at once. */
export const BARS = 12;

export interface Engine {
  state: PipelineState;
  /** Newest last. Values are 0..1. */
  levels: number[];
  text: string;
  error: string | null;
  /** Seconds since recording started, for the counter under the status. */
  elapsed: number;
}

const QUIET: number[] = Array.from({ length: BARS }, () => 0);

export function useEngine(): Engine {
  const [state, setState] = useState<PipelineState>("idle");
  const [levels, setLevels] = useState<number[]>(QUIET);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);

  const startedAt = useRef<number | null>(null);

  useEffect(() => {
    const pending = listen<EngineEvent>(ENGINE_EVENT, ({ payload }) => {
      switch (payload.kind) {
        case "state":
          setState(payload.state);
          if (payload.state === "recording") {
            // A new dictation starts clean; leaving the last one's text up
            // while the bars move would be a lie about what is happening.
            startedAt.current = performance.now();
            setText("");
            setError(null);
            setLevels(QUIET);
            setElapsed(0);
          }
          if (payload.state === "idle") {
            startedAt.current = null;
          }
          break;
        case "level":
          setLevels((previous) => [...previous.slice(1), payload.peak]);
          break;
        case "text":
          setText(payload.text);
          break;
        case "failed":
          setError(payload.message);
          break;
        case "loading":
        case "ready":
          break;
      }
    });

    return () => {
      void pending.then((unlisten) => {
        unlisten();
      });
    };
  }, []);

  // The counter ticks only while recording, and only ten times a second —
  // it is rendered with tabular figures and shows tenths at most.
  useEffect(() => {
    if (state !== "recording") return;
    const timer = window.setInterval(() => {
      if (startedAt.current !== null) {
        setElapsed((performance.now() - startedAt.current) / 1000);
      }
    }, 100);
    return () => {
      window.clearInterval(timer);
    };
  }, [state]);

  return { state, levels, text, error, elapsed };
}
