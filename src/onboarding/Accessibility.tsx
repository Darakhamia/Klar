/**
 * Step two — accessibility, on macOS only.
 *
 * Windows needs no grant for the low-level hook or for SendInput, so
 * `permission_state` reports `notApplicable` there and this screen is not in
 * the list at all. It is not shown and skipped; it is absent.
 *
 * There is no way to grant this from inside the app — macOS requires the user
 * to tick the box themselves — so the screen opens the pane and then waits,
 * checking until the answer changes.
 */

import { useEffect, useState } from "react";
import { openPermissionSettings, permissionStates } from "../lib/ipc";

/** How often to ask the OS again while waiting. Cheap, and the wait is the
 * user walking to another window and back. */
const POLL_MS = 1000;

export function Accessibility({ onGranted }: { onGranted: () => void }) {
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const timer = window.setInterval(() => {
      permissionStates()
        .then((states) => {
          if (cancelled) return;
          const granted = states.some(
            (report) => report.permission === "accessibility" && report.state === "granted",
          );
          if (granted) onGranted();
        })
        .catch(() => {
          // A failed check is not worth a message: the next one is a second
          // away, and the screen already says what it is waiting for.
        });
    }, POLL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [onGranted]);

  return (
    <>
      <div className="ob-inject">
        <div className="ob-inject__app">Klar</div>
        <div className="ob-inject__arrow" />
        <div className="ob-inject__target">
          Ship on Friday.<span className="ob-inject__caret">|</span>
        </div>
      </div>

      <h2>One more permission, for typing</h2>
      <p className="ob__body">
        macOS treats typing into another app as accessibility control, so it has to be granted by
        hand. Klar uses it for exactly one thing: putting finished text where your cursor already is.
      </p>
      <p className="ob__aside">
        It does not read your screen, your keystrokes, or any other app&apos;s contents.
      </p>

      {error && <p className="ob__error">{error}</p>}

      <div className="ob__actions">
        <button
          type="button"
          className="ob__btn ob__btn--primary"
          onClick={() => {
            openPermissionSettings("accessibility").catch((cause: unknown) => {
              setError(cause instanceof Error ? cause.message : String(cause));
            });
          }}
        >
          Open accessibility settings
        </button>
        <div className="label">Waiting for permission</div>
      </div>
    </>
  );
}
