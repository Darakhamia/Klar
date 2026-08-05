/**
 * The only way the frontend reaches Rust.
 *
 * Every command is declared here with its return type so the rest of the app
 * never calls `invoke` with a bare string. No business logic lives on this
 * side — these are transport wrappers and nothing else.
 */

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface AppVersion {
  version: string;
  os: string;
  arch: string;
}

export type Modifier = "control" | "alt" | "shift" | "meta";
export type Key =
  | "space"
  | "fn"
  | "f1"
  | "f2"
  | "f3"
  | "f4"
  | "f5"
  | "f6"
  | "f7"
  | "f8"
  | "f9"
  | "f10"
  | "f11"
  | "f12";

export interface Binding {
  modifiers: Modifier[];
  key: Key;
}

export type Permission = "microphone" | "accessibility";
export type PermissionState = "granted" | "denied" | "unknown" | "notApplicable";

export interface PermissionReport {
  permission: Permission;
  state: PermissionState;
}

export type PipelineState =
  | "idle"
  | "recording"
  | "transcribing"
  | "polishing"
  | "injecting"
  | "error";

/** The engine's one channel, mirroring `UiEvent` in `src-tauri/src/engine.rs`.
 * Every window that shows what the pipeline is doing reads this and nothing
 * else — no window infers a state the Rust side did not send. */
export type EngineEvent =
  | { kind: "state"; state: PipelineState }
  | { kind: "level"; peak: number }
  | { kind: "text"; text: string; settled: boolean }
  | { kind: "failed"; message: string }
  | { kind: "loading"; what: string }
  | { kind: "ready"; hotkey: Binding };

export const ENGINE_EVENT = "klar://event";

/** Download progress, mirroring `DownloadEvent` in
 * `src-tauri/src/downloads.rs`. */
export type ModelEvent =
  | { phase: "downloading"; id: string; received: number; total: number }
  | { phase: "verifying"; id: string }
  | { phase: "done"; id: string }
  | { phase: "failed"; id: string; message: string };

export const MODEL_EVENT = "klar://model";

/** The answer to a `hotkeyCapture`, mirroring `CaptureResult` in
 * `src-tauri/src/commands.rs`. Rust has already saved a bound chord and
 * restarted the engine on it by the time this arrives. */
export type CaptureResult =
  | { outcome: "bound"; binding: Binding }
  | { outcome: "refused"; message: string };

export const HOTKEY_EVENT = "klar://hotkey";

/** Carries the whole `Settings` whenever Rust changes them on its own
 * initiative — today, only after capturing a new hotkey. A window that made the
 * change itself already knows, so `settings_set` deliberately does not
 * broadcast: the echo would race the next change. */
export const SETTINGS_EVENT = "klar://settings";

/** True when running inside the Tauri shell rather than a plain browser tab. */
export const inShell = (): boolean => isTauri();

/**
 * Subscribe to an event, and return the cleanup an effect wants.
 *
 * `listen` can be refused — it is one of the core commands the capability files
 * gate, so a window missing from `capabilities/default.json` gets a rejected
 * promise and nothing else. Writing that as `void pending.then(unlisten)`
 * throws the refusal away, which is how two windows shipped subscribing to an
 * event channel that was never going to reach them. Anything that fails here is
 * loud.
 */
export function subscribe<T>(
  event: string,
  handler: (payload: T) => void,
  onError?: (message: string) => void,
): () => void {
  let cancelled = false;
  let stop: (() => void) | null = null;

  listen<T>(event, ({ payload }) => {
    handler(payload);
  })
    .then((unlisten) => {
      if (cancelled) unlisten();
      else stop = unlisten;
    })
    .catch((cause: unknown) => {
      const message = cause instanceof Error ? cause.message : String(cause);
      console.error(`klar: could not subscribe to ${event}`, message);
      onError?.(message);
    });

  return () => {
    cancelled = true;
    stop?.();
  };
}

export const appVersion = (): Promise<AppVersion> => invoke<AppVersion>("app_version");

export const defaultHotkey = (): Promise<Binding> => invoke<Binding>("default_hotkey");

export const permissionStates = (): Promise<PermissionReport[]> =>
  invoke<PermissionReport[]>("permission_states");

export const openPermissionSettings = (permission: Permission): Promise<void> =>
  invoke<void>("open_permission_settings", { permission });

/** Start fetching a model. Progress arrives on {@link MODEL_EVENT}. */
export const downloadModel = (id: string): Promise<void> => invoke<void>("model_download", { id });

/** Open the microphone and report levels on {@link ENGINE_EVENT} until stopped. */
export const micTestStart = (): Promise<void> => invoke<void>("mic_test_start");

export const micTestStop = (): Promise<void> => invoke<void>("mic_test_stop");

/** Start the engine again on whatever models are now on disk. */
export const restartEngine = (): Promise<void> => invoke<void>("engine_restart");

export const finishOnboarding = (): Promise<void> => invoke<void>("onboarding_finish");

/** Wait for the user to press a chord and bind it. The answer arrives on
 * {@link HOTKEY_EVENT}; the engine is stopped for the duration. */
export const captureHotkey = (): Promise<void> => invoke<void>("hotkey_capture");

const MODIFIER_GLYPHS: Record<Modifier, { mac: string; other: string }> = {
  control: { mac: "⌃", other: "Ctrl" },
  alt: { mac: "⌥", other: "Alt" },
  shift: { mac: "⇧", other: "Shift" },
  meta: { mac: "⌘", other: "Win" },
};

const KEY_LABELS: Record<Key, string> = {
  space: "Space",
  fn: "fn",
  f1: "F1",
  f2: "F2",
  f3: "F3",
  f4: "F4",
  f5: "F5",
  f6: "F6",
  f7: "F7",
  f8: "F8",
  f9: "F9",
  f10: "F10",
  f11: "F11",
  f12: "F12",
};

/** Render a binding the way the design writes it: `CTRL SPACE`, `⌥ SPACE`. */
export function formatBinding(binding: Binding, os: string): string {
  const mac = os === "macos";
  const parts = binding.modifiers.map((m) =>
    mac ? MODIFIER_GLYPHS[m].mac : MODIFIER_GLYPHS[m].other,
  );
  parts.push(KEY_LABELS[binding.key]);
  return parts.join(mac ? " " : " + ").toUpperCase();
}
