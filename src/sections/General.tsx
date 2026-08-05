import { useEffect, useState } from "react";
import { Figure, Row, Segmented, Select } from "../components/Row";
import {
  formatBinding,
  note,
  revealLog,
  setHotkey,
  suspendHotkey,
  type Binding,
  type Modifier,
} from "../lib/ipc";
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

      <Row
        label="Diagnostics"
        hint="Klar keeps a log on this machine and sends nothing anywhere. If something goes wrong, this is the file to attach."
      >
        <RevealLog />
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

/** The keys that only ever build a chord, never complete one. */
const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

function heldModifiers(event: KeyboardEvent): Modifier[] {
  const held: Modifier[] = [];
  if (event.ctrlKey) held.push("control");
  if (event.altKey) held.push("alt");
  if (event.shiftKey) held.push("shift");
  if (event.metaKey) held.push("meta");
  return held;
}

/**
 * The push-to-talk binding, and rebinding it.
 *
 * The chord is read from this window's own keyboard events, not from the
 * global hook. It has to be: while Klar's window has focus the hook is not
 * handed keystrokes at all — which is exactly the moment somebody is choosing a
 * hotkey — and a window that has focus is handed its own key events by
 * definition. Rust is told the `KeyboardEvent.code` and which modifiers were
 * down; what that key is, and whether it can be bound, is decided there.
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

  useEffect(() => {
    if (!capturing) return;

    const stop = () => {
      setCapturing(false);
      void suspendHotkey(false);
    };

    const onKeyDown = (event: KeyboardEvent) => {
      // Nothing typed while choosing a hotkey should reach the page, including
      // the chords the webview would otherwise act on itself.
      event.preventDefault();
      event.stopPropagation();

      if (event.code === "Escape") {
        stop();
        return;
      }
      // Still building the chord.
      if (MODIFIER_CODES.has(event.code)) return;

      note("settings", `rebind: read ${event.code}`);
      setHotkey(event.code, heldModifiers(event))
        .then((hotkey) => {
          setRefused(null);
          onRebound(hotkey);
          stop();
        })
        .catch((cause: unknown) => {
          // Refusals leave the capture running: the user is mid-decision, and
          // taking the field away from them to make them press the button again
          // would be the wrong answer to "that key needs a modifier".
          setRefused(cause instanceof Error ? cause.message : String(cause));
        });
    };

    window.addEventListener("keydown", onKeyDown, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
    };
  }, [capturing, onRebound]);

  const hint = capturing
    ? (refused ?? "Press any key, with Ctrl, Alt, Shift or Win held. Escape cancels.")
    : (refused ?? "Hold to dictate. Release to insert.");

  return (
    <Row label="Dictation hotkey" hint={hint} alert={refused !== null}>
      <Figure>{formatBinding(settings.hotkey, os)}</Figure>
      <button
        type="button"
        className="btn btn--ghost"
        onClick={() => {
          if (capturing) {
            setCapturing(false);
            void suspendHotkey(false);
            return;
          }
          setRefused(null);
          suspendHotkey(true)
            .then(() => {
              setCapturing(true);
            })
            .catch((cause: unknown) => {
              setRefused(cause instanceof Error ? cause.message : String(cause));
            });
        }}
      >
        {capturing ? "Press a key…" : "Change"}
      </button>
    </Row>
  );
}

/** Opens the file manager with the newest log selected. */
function RevealLog() {
  const [failed, setFailed] = useState<string | null>(null);

  return (
    <button
      type="button"
      className="btn"
      onClick={() => {
        setFailed(null);
        revealLog().catch((cause: unknown) => {
          setFailed(cause instanceof Error ? cause.message : String(cause));
        });
      }}
    >
      {failed ?? "Show log file"}
    </button>
  );
}
