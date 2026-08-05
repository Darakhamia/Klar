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

/** Mirrors `Key` in `crates/klar-platform/src/hotkey.rs`. Serde writes the unit
 * variants as strings and the two that carry a value as one-key objects. */
export type Key = "space" | "fn" | { function: number } | { character: string };

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

/** Wait for the user to press a chord and bind it, resolving with what was
 * bound. Rejects with the refusal — timed out, cancelled, or a chord Klar will
 * not take — written for a person. Takes as long as the user does. */
export const captureHotkey = (): Promise<Binding> => invoke<Binding>("hotkey_capture");

const MODIFIER_GLYPHS: Record<Modifier, { mac: string; other: string }> = {
  control: { mac: "⌃", other: "Ctrl" },
  alt: { mac: "⌥", other: "Alt" },
  shift: { mac: "⇧", other: "Shift" },
  meta: { mac: "⌘", other: "Win" },
};

function keyLabel(key: Key): string {
  if (key === "space") return "Space";
  if (key === "fn") return "fn";
  if ("function" in key) return `F${String(key.function)}`;
  return key.character.toUpperCase();
}

/** Render a binding the way the design writes it: `CTRL SPACE`, `⌥ SPACE`. */
export function formatBinding(binding: Binding, os: string): string {
  const mac = os === "macos";
  const parts = binding.modifiers.map((m) =>
    mac ? MODIFIER_GLYPHS[m].mac : MODIFIER_GLYPHS[m].other,
  );
  parts.push(keyLabel(binding.key));
  return parts.join(mac ? " " : " + ").toUpperCase();
}
