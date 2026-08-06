/**
 * The settings window.
 *
 * 900 × 620 and completely still. Settings are a place you visit twice a year,
 * so nothing here moves or celebrates: rows sit on a single left edge, 2px
 * rules separate them, and red marks only what is currently on.
 */

import { useCallback, useEffect, useState } from "react";
import {
  ENGINE_EVENT,
  appVersion,
  inShell,
  note,
  subscribe,
  type AppVersion,
  type Binding,
  type EngineEvent,
} from "./lib/ipc";
import {
  getSettings,
  listDevices,
  listModels,
  setSettings,
  type Device,
  type ModelStatus,
  type Settings,
} from "./lib/settings";
import { applyAppearance } from "./lib/theme";
import { General } from "./sections/General";
import { Voice } from "./sections/Voice";
import { Dictionary } from "./sections/Dictionary";
import { History } from "./sections/History";
import { Stats } from "./sections/Stats";
import "./App.css";

const SECTIONS = ["General", "Voice", "Dictionary", "History", "Stats"] as const;
type Section = (typeof SECTIONS)[number];

export function App() {
  const [section, setSection] = useState<Section>("General");
  const [settings, setLocal] = useState<Settings | null>(null);
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [version, setVersion] = useState<AppVersion | null>(null);
  const [level, setLevel] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = inShell()
      ? Promise.all([getSettings(), listModels(), listDevices(), appVersion()])
      : Promise.reject(
          new Error("Not running inside the Klar shell — start it with `npm run tauri dev`."),
        );
    load
      .then(([loaded, catalogue, inputs, app]) => {
        if (cancelled) return;
        applyAppearance(loaded.appearance);
        setLocal(loaded);
        setModels(catalogue);
        setDevices(inputs);
        setVersion(app);
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // The meter in Voice is fed by the engine's own level events, so there is no
  // second audio stream open just to draw a bar.
  useEffect(() => {
    if (!inShell()) return;
    note("settings", "mounted; subscribing to the engine");
    let heard = false;

    return subscribe<EngineEvent>(
      ENGINE_EVENT,
      (payload) => {
        // Once, on the first event that ever arrives. The open question is
        // whether any do — this window has been acting as though none reach it.
        if (!heard) {
          heard = true;
          note("settings", `first engine event received: ${payload.kind}`);
        }
        if (payload.kind === "level") setLevel(payload.peak);
        if (payload.kind === "state") setLevel(0);
      },
      // A refused subscription used to reach nothing but the console, which is
      // how a window sat deaf for a milestone. It is an error like any other.
      (message) => {
        note("settings", `subscribe refused: ${message}`);
        setError(message);
      },
    );
  }, []);

  // A rebind is already saved and applied by the time it answers, so this only
  // brings the window's copy up to date — otherwise the next save would write
  // the old binding back over it.
  const rebound = useCallback((hotkey: Binding) => {
    setLocal((previous) => (previous ? { ...previous, hotkey } : previous));
  }, []);

  // Every change is saved and applied immediately. There is no Save button:
  // nothing here is a form, and a setting that has not taken effect is a lie.
  const update = useCallback((next: Settings) => {
    applyAppearance(next.appearance);
    setLocal(next);
    setSettings(next).catch((cause: unknown) => {
      setError(cause instanceof Error ? cause.message : String(cause));
    });
  }, []);

  return (
    <main className="window">
      <nav className="nav">
        <div className="nav__brand">Klar</div>
        {SECTIONS.map((name) => (
          <button
            key={name}
            type="button"
            className="nav__item"
            aria-current={name === section ? "page" : undefined}
            onClick={() => {
              setSection(name);
            }}
          >
            {name}
          </button>
        ))}
        <div className="nav__foot figure">
          {version ? `${version.version} · ${version.os} ${version.arch}` : ""}
        </div>
      </nav>

      <section className="panel">
        <header className="panel__head">
          <h1>{section}</h1>
        </header>

        {error && <p className="panel__error">{error}</p>}

        <div className="panel__body">
          {settings && section === "General" && (
            <General
              settings={settings}
              os={version?.os ?? "windows"}
              onChange={update}
              onRebound={rebound}
            />
          )}
          {settings && section === "Voice" && (
            <Voice
              settings={settings}
              models={models}
              devices={devices}
              level={level}
              onChange={update}
            />
          )}
          {section === "Dictionary" && <Dictionary />}
          {section === "History" && <History />}
          {section === "Stats" && <Stats />}
          {!settings && !error && <p className="panel__error">Reading settings…</p>}
        </div>
      </section>
    </main>
  );
}
