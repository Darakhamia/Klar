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
  subscribe,
  type AppVersion,
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
import { General } from "./sections/General";
import { Voice } from "./sections/Voice";
import { DICTIONARY, HISTORY, Pending, STATS } from "./sections/Pending";
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
    return subscribe<EngineEvent>(ENGINE_EVENT, (payload) => {
      if (payload.kind === "level") setLevel(payload.peak);
      if (payload.kind === "state") setLevel(0);
    });
  }, []);

  // Every change is saved and applied immediately. There is no Save button:
  // nothing here is a form, and a setting that has not taken effect is a lie.
  const update = useCallback((next: Settings) => {
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
            <General settings={settings} os={version?.os ?? "windows"} onChange={update} />
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
          {section === "Dictionary" && <Pending {...DICTIONARY} />}
          {section === "History" && <Pending {...HISTORY} />}
          {section === "Stats" && <Pending {...STATS} />}
          {!settings && !error && <p className="panel__error">Reading settings…</p>}
        </div>
      </section>
    </main>
  );
}
