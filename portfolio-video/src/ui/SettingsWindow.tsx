/**
 * The settings window.
 *
 * The shell is `App.tsx`'s markup, copied with its class names: App itself
 * loads settings over IPC on mount and refuses to run outside the Tauri shell.
 * The sections inside it are the real `History` and `Voice` components, fed
 * by src/mocks/settings.ts.
 */

import { DEVICES, MODELS, SETTINGS, VERSION } from "../demo";
import { History } from "../../../src/sections/History";
import { Voice } from "../../../src/sections/Voice";
import { Controls } from "./Controls";

const SECTIONS = ["General", "Voice", "Dictionary", "History", "Stats"] as const;

export type Section = "History" | "Voice";

export function SettingsWindow({ section }: { section: Section }) {
  return (
    <div className="kv-win">
      <div className="kv-win__bar">
        <span>Klar</span>
        <Controls />
      </div>
      <div className="kv-win__body">
        <main className="window">
          <nav className="nav">
            <div className="nav__brand">Klar</div>
            {SECTIONS.map((name) => (
              <button
                key={name}
                type="button"
                className="nav__item"
                aria-current={name === section ? "page" : undefined}
              >
                {name}
              </button>
            ))}
            <div className="nav__foot figure">{VERSION}</div>
          </nav>

          <section className="panel">
            <header className="panel__head">
              <h1>{section}</h1>
            </header>

            <div className="panel__body">
              {section === "Voice" && (
                <Voice
                  settings={SETTINGS}
                  models={MODELS}
                  devices={DEVICES}
                  level={0}
                  onChange={() => undefined}
                />
              )}
              {section === "History" && <History />}
            </div>
          </section>
        </main>
      </div>
    </div>
  );
}
