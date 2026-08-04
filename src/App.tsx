/**
 * M0 shell.
 *
 * The window exists and can read the Rust side; the five real sections
 * (General, Voice, Dictionary, History, Stats) arrive in M6. Everything shown
 * here comes from a named command — nothing is hard-coded on this side.
 */

import { useEffect, useState } from "react";
import {
  appVersion,
  defaultHotkey,
  formatBinding,
  inShell,
  permissionStates,
  type AppVersion,
  type Binding,
  type PermissionReport,
} from "./lib/ipc";
import "./App.css";

interface Backend {
  version: AppVersion;
  hotkey: Binding;
  permissions: PermissionReport[];
}

export function App() {
  const [backend, setBackend] = useState<Backend | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = inShell()
      ? Promise.all([appVersion(), defaultHotkey(), permissionStates()])
      : Promise.reject(
          new Error("Not running inside the Klar shell — start it with `npm run tauri dev`."),
        );
    load
      .then(([version, hotkey, permissions]) => {
        if (!cancelled) setBackend({ version, hotkey, permissions });
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="shell">
      <header className="shell__head">
        <div className="label">Klar — desktop dictation</div>
        <h1>Messy speech in, finished text out.</h1>
      </header>

      {error && <p className="shell__note shell__note--error">{error}</p>}

      {backend && (
        <dl className="facts">
          <Fact term="Version" value={backend.version.version} />
          <Fact term="Platform" value={`${backend.version.os} ${backend.version.arch}`} />
          <Fact
            term="Push to talk"
            value={formatBinding(backend.hotkey, backend.version.os)}
          />
          {backend.permissions.map((report) => (
            <Fact key={report.permission} term={report.permission} value={report.state} />
          ))}
        </dl>
      )}

      {!backend && !error && <p className="shell__note">Reading the backend…</p>}

      <footer className="shell__foot">
        <span className="label">M0 — scaffold</span>
        <span className="label">Speech never leaves this machine</span>
      </footer>
    </main>
  );
}

function Fact({ term, value }: { term: string; value: string }) {
  return (
    <div className="facts__row">
      <dt className="label">{term}</dt>
      <dd className="figure">{value}</dd>
    </div>
  );
}
