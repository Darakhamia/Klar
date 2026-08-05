/**
 * First run.
 *
 * Three screens on macOS, two on Windows — the accessibility grant has no
 * Windows equivalent, so that step is absent rather than shown and skipped, and
 * the indicator reads "1 of 2". Nothing is explained twice, and there is no
 * skip on the permission step because the app cannot work without it.
 */

import { useCallback, useEffect, useState } from "react";
import { appVersion, finishOnboarding, inShell, permissionStates } from "../lib/ipc";
import { getSettings, type Settings } from "../lib/settings";
import { applyAppearance } from "../lib/theme";
import { Accessibility } from "./Accessibility";
import { Microphone } from "./Microphone";
import { Setup } from "./Setup";
import { useEngine } from "./useEngine";
import "./onboarding.css";

type Step = "microphone" | "accessibility" | "setup";

export function Onboarding() {
  const engine = useEngine();

  const [steps, setSteps] = useState<Step[] | null>(null);
  const [at, setAt] = useState(0);
  const [os, setOs] = useState("windows");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = inShell()
      ? Promise.all([appVersion(), permissionStates(), getSettings()])
      : Promise.reject(
          new Error("Not running inside the Klar shell — start it with `npm run tauri dev`."),
        );

    load
      .then(([app, permissions, loaded]) => {
        if (cancelled) return;
        // The steps come from what this OS actually gates, not from a constant
        // with a platform check next to it.
        const gated = permissions.some(
          (report) => report.permission === "accessibility" && report.state !== "notApplicable",
        );
        applyAppearance(loaded.appearance);
        setOs(app.os);
        setSettings(loaded);
        setSteps(gated ? ["microphone", "accessibility", "setup"] : ["microphone", "setup"]);
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause));
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const next = useCallback(() => {
    setAt((index) => index + 1);
  }, []);

  const done = () => {
    finishOnboarding().catch((cause: unknown) => {
      setError(cause instanceof Error ? cause.message : String(cause));
    });
  };

  if (error) {
    return (
      <main className="ob">
        <p className="ob__error">{error}</p>
      </main>
    );
  }

  if (steps === null || settings === null) {
    return <main className="ob" />;
  }

  const step = steps[Math.min(at, steps.length - 1)];

  return (
    <main className="ob">
      <Progress at={Math.min(at, steps.length - 1)} total={steps.length} />

      {step === "microphone" && <Microphone engine={engine} onContinue={next} />}
      {step === "accessibility" && <Accessibility onGranted={next} />}
      {step === "setup" && <Setup settings={settings} os={os} engine={engine} onDone={done} />}
    </main>
  );
}

/** The rule of dashes at the top. One per step, red for the ones behind you. */
function Progress({ at, total }: { at: number; total: number }) {
  return (
    <div className="ob-steps">
      {Array.from({ length: total }, (_, index) => (
        <span key={index} className="ob-steps__dash" data-on={index <= at ? "true" : "false"} />
      ))}
      <div className="label">
        {at + 1} of {total}
      </div>
    </div>
  );
}
