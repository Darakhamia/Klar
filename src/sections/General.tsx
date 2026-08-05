import { useEffect, useState } from "react";
import { Figure, Row, Segmented, Select } from "../components/Row";
import {
  HOTKEY_EVENT,
  captureHotkey,
  formatBinding,
  subscribe,
  type CaptureResult,
} from "../lib/ipc";
import type { Appearance, FinishAction, OverlayPosition, Settings } from "../lib/settings";

export function General({
  settings,
  os,
  onChange,
}: {
  settings: Settings;
  os: string;
  onChange: (next: Settings) => void;
}) {
  return (
    <>
      <Hotkey settings={settings} os={os} />

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
 * No `onChange` here: Rust saves the captured chord and restarts the engine on
 * it before it tells anyone, and the settings broadcast that follows is what
 * updates this row. Saving it again from here would restart the engine twice.
 */
function Hotkey({ settings, os }: { settings: Settings; os: string }) {
  const [capturing, setCapturing] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  useEffect(
    () =>
      subscribe<CaptureResult>(HOTKEY_EVENT, (result) => {
        setCapturing(false);
        setRefused(result.outcome === "refused" ? result.message : null);
      }),
    [],
  );

  // While capturing there is no hotkey at all — the engine is stopped so its
  // hook cannot race the capture — so the row says what to do rather than
  // leaving the old binding looking live.
  const hint = capturing
    ? "Hold a modifier and press a letter, a digit, Space or an F-key. Escape cancels."
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
          captureHotkey().catch((cause: unknown) => {
            setCapturing(false);
            setRefused(cause instanceof Error ? cause.message : String(cause));
          });
        }}
      >
        {capturing ? "Press a key…" : "Change"}
      </button>
    </Row>
  );
}
