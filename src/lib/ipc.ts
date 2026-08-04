/**
 * The only way the frontend reaches Rust.
 *
 * Every command is declared here with its return type so the rest of the app
 * never calls `invoke` with a bare string. No business logic lives on this
 * side — these are transport wrappers and nothing else.
 */

import { invoke, isTauri } from "@tauri-apps/api/core";

export interface AppVersion {
  version: string;
  os: string;
  arch: string;
}

export type Modifier = "control" | "alt" | "shift" | "meta";
export type Key = "space" | "fn" | "f1" | "f2" | "f3" | "f4";

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

/** True when running inside the Tauri shell rather than a plain browser tab. */
export const inShell = (): boolean => isTauri();

export const appVersion = (): Promise<AppVersion> => invoke<AppVersion>("app_version");

export const defaultHotkey = (): Promise<Binding> => invoke<Binding>("default_hotkey");

export const permissionStates = (): Promise<PermissionReport[]> =>
  invoke<PermissionReport[]>("permission_states");

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
};

/** Render a binding the way the design writes it: `CTRL SPACE`, `⌥ SPACE`. */
export function formatBinding(binding: Binding, os: string): string {
  const mac = os === "macos";
  const parts = binding.modifiers.map((m) => (mac ? MODIFIER_GLYPHS[m].mac : MODIFIER_GLYPHS[m].other));
  parts.push(KEY_LABELS[binding.key]);
  return parts.join(mac ? " " : " + ").toUpperCase();
}
