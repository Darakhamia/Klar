import { Figure, Row, Segmented, Select } from "../components/Row";
import { formatBinding } from "../lib/ipc";
import type { FinishAction, OverlayPosition, Settings } from "../lib/settings";

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
      <Row label="Dictation hotkey" hint="Hold to dictate. Release to insert.">
        <Figure>{formatBinding(settings.hotkey, os)}</Figure>
        {/* Rebinding needs the hook to capture a chord rather than act on it,
            which is a mode the platform layer does not have yet. */}
        <button type="button" className="btn btn--ghost" disabled>
          Change
        </button>
      </Row>

      <Row label="Launch at login" hint="Klar starts hidden in the system tray.">
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
