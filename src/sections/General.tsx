import { useState } from "react";
import { Figure, Row, Segmented, Select } from "../components/Row";
import { captureHotkey, formatBinding, note, type Binding } from "../lib/ipc";
import type { Appearance, FinishAction, OverlayPosition, Settings } from "../lib/settings";

export function General({
  settings,
  os,
  onChange,
  onRebound,
}: {
  settings: Settings;
  os: string;
  onChange: (next: Settings) => void;
  onRebound: (hotkey: Binding) => void;
}) {
  return (
    <>
      <Hotkey settings={settings} os={os} onRebound={onRebound} />

      <Row
        label={os === "macos" ? "Launch at login" : "Start with Windows"}
        hint="Klar starts hidden in the tray."
      >
        <Segmented<"on" | "off">
          value={settings.launchAtLogin ? "on" : "off"}
          options={[
            { value: "on", label: "On" },
            { value: "off", label: "Off" },
          ]}
          onChange={(value) => {
            onChange({ ...settings, launchAtLogin: value === "on" });
          }}
        />
      </Row>

      <Row label="When dictation ends" hint="Where the finished text goes.">
        <Segmented<FinishAction>
          value={settings.onFinish}
          options={[
            { value: "type", label: "Type at the cursor" },
            { value: "copy", label: "Copy to the clipboard" },
            { value: "typeAndCopy", label: "Type and copy" },
          ]}
          onChange={(onFinish) => {
            onChange({ ...settings, onFinish });
          }}
        />
      </Row>

      <Row label="Appearance" hint="System follows what the OS is set to.">
        <Segmented<Appearance>
          value={settings.appearance}
          options={[
            { value: "system", label: "System" },
            { value: "light", label: "Light" },
            { value: "dark", label: "Dark" },
          ]}
          onChange={(appearance) => {
            onChange({ ...settings, appearance });
          }}
        />
      </Row>

      <Row label="Overlay position" hint="Where the overlay appears while you speak.">
        <Select<OverlayPosition>
          value={settings.overlayPosition}
          options={[
            { value: "bottomCentre", label: "Bottom of the screen" },
            { value: "topCentre", label: "Top of the screen" },
            { value: "nearCursor", label: "Near the cursor" },
          ]}
          onChange={(overlayPosition) => {
            onChange({ ...settings, overlayPosition });
          }}
        />
      </Row>
    </>
  );
}

/**
 * The push-to-talk binding, and rebinding it.
 *
 * The whole exchange is one command call: it resolves with the chord that was
 * bound, or rejects with the reason it was not. Rust has already saved it and
 * is restarting the engine on it by then, so there is nothing to save from
 * here — `onRebound` only tells the window above what it now holds, so that a
 * later save does not write the old binding back.
 */
function Hotkey({
  settings,
  os,
  onRebound,
}: {
  settings: Settings;
  os: string;
  onRebound: (hotkey: Binding) => void;
}) {
  const [capturing, setCapturing] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  // While capturing, the next key pressed anywhere is swallowed, so the row
  // says what to do rather than leaving the old binding looking live.
  const hint = capturing
    ? "Press any key, with Ctrl, Alt, Shift or Win held. Function keys need no modifier. Escape cancels."
    : (refused ?? "Hold to dictate. Release to insert.");

  return (
    <Row label="Dictation hotkey" hint={hint} alert={refused !== null && !capturing}>
      <Figure>{formatBinding(settings.hotkey, os)}</Figure>
      <button
        type="button"
        className="btn btn--ghost"
        disabled={capturing}
        onClick={() => {
          setRefused(null);
          setCapturing(true);
          note("settings", "rebind: asked, button should now read Press a key…");
          captureHotkey()
            .then((hotkey) => {
              note("settings", `rebind: resolved with ${JSON.stringify(hotkey)}`);
              onRebound(hotkey);
            })
            .catch((cause: unknown) => {
              const message = cause instanceof Error ? cause.message : String(cause);
              note("settings", `rebind: rejected with ${message}`);
              setRefused(message);
            })
            .finally(() => {
              setCapturing(false);
            });
        }}
      >
        {capturing ? "Press a key…" : "Change"}
      </button>
    </Row>
  );
}
