/**
 * Step one — the microphone.
 *
 * Windows has no permission API worth asking: a desktop app is not gated by
 * the Privacy toggle the way a packaged one is, so `permission_state` answers
 * `Unknown` and means it. The honest test is to open the device, which is what
 * the button does. The bars then answer the only question this screen is really
 * asking, which is whether Klar can hear you.
 */

import { useEffect, useState } from "react";
import { micTestStart, micTestStop, openPermissionSettings } from "../lib/ipc";
import { BARS, type EngineView } from "./useEngine";

export function Microphone({ engine, onContinue }: { engine: EngineView; onContinue: () => void }) {
  const [listening, setListening] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Whichever way this screen is left, the stream closes with it.
  useEffect(
    () => () => {
      void micTestStop();
    },
    [],
  );

  const open = () => {
    setError(null);
    micTestStart()
      .then(() => {
        setListening(true);
      })
      .catch((cause: unknown) => {
        setError(cause instanceof Error ? cause.message : String(cause));
      });
  };

  return (
    <>
      <Wave levels={engine.levels} live={listening} />

      <h2>Let Klar hear you</h2>
      <p className="ob__body">
        Windows may ask you to confirm. Audio stays on this PC — it goes to the speech model on your
        machine and nowhere else.
      </p>

      {error && (
        <p className="ob__error">
          The microphone would not open: {error}
          <br />
          Check that a microphone is plugged in and that Klar is allowed to use it.
        </p>
      )}

      <div className="ob__actions">
        {listening ? (
          <button type="button" className="ob__btn ob__btn--primary" onClick={onContinue}>
            Continue
          </button>
        ) : (
          <button type="button" className="ob__btn ob__btn--primary" onClick={open}>
            Allow microphone
          </button>
        )}

        {listening ? (
          <div className="label">{engine.heard ? "Klar can hear you" : "Say something"}</div>
        ) : (
          <button
            type="button"
            className="ob__btn"
            onClick={() => {
              void openPermissionSettings("microphone");
            }}
          >
            Open privacy settings
          </button>
        )}
      </div>
    </>
  );
}

/** The design's eight bars. Idle they breathe on a loop; live they are the
 * real input level, so the difference between the two is the whole point. */
function Wave({ levels, live }: { levels: number[]; live: boolean }) {
  return (
    <div className="ob-wave" aria-label="input level">
      {Array.from({ length: BARS }, (_, index) => {
        const level = levels[index] ?? 0;
        // Square root, as everywhere else in Klar: speech sits low in a linear
        // scale and a linear meter looks broken.
        const height = Math.max(0.16, Math.min(1, Math.sqrt(level)));
        return (
          <span
            key={index}
            className={live ? "ob-wave__bar" : "ob-wave__bar ob-wave__bar--idle"}
            style={
              live
                ? { transform: `scaleY(${String(height)})` }
                : { animationDelay: `${String(index * 0.09)}s` }
            }
          />
        );
      })}
    </div>
  );
}
