/**
 * The settings the Rust side stores, and the commands that read and write them.
 *
 * These types mirror `src-tauri/src/settings.rs`. They are the only shape the
 * window is allowed to invent — every value shown comes from there.
 */

import { invoke } from "@tauri-apps/api/core";
import type { Binding } from "./ipc";

export type FinishAction = "type" | "copy" | "typeAndCopy";
export type OverlayPosition = "bottomCentre" | "nearCursor" | "topCentre";
export type Processing = "local" | "cloud";
export type Cleanup = "verbatim" | "light" | "balanced" | "heavy";
export type Appearance = "system" | "light" | "dark";

export interface Settings {
  hotkey: Binding;
  launchAtLogin: boolean;
  onFinish: FinishAction;
  overlayPosition: OverlayPosition;
  language: string | null;
  processing: Processing;
  model: string;
  microphone: string | null;
  cleanup: Cleanup;
  polishEndpoint: string;
  polishModel: string;
  appearance: Appearance;
  onboarded: boolean;
}

export interface ModelStatus {
  id: string;
  fileName: string;
  url: string;
  sha256: string;
  bytes: number;
  multilingual: boolean;
  summary: string;
  installed: boolean;
  size: string;
}

export interface Device {
  name: string;
  isDefault: boolean;
}

export const getSettings = (): Promise<Settings> => invoke<Settings>("settings_get");

export const setSettings = (settings: Settings): Promise<void> =>
  invoke<void>("settings_set", { settings });

export const listModels = (): Promise<ModelStatus[]> => invoke<ModelStatus[]>("models");

export const listDevices = (): Promise<Device[]> => invoke<Device[]>("audio_devices");

/** What the local model server has, if it is running. `models` is empty when it
 * is not, which is the same answer either way: nothing to choose from. */
export interface PolishStatus {
  reachable: boolean;
  endpoint: string;
  models: string[];
}

export const polishStatus = (): Promise<PolishStatus> => invoke<PolishStatus>("polish_status");

/** A compute device ggml found — a graphics card, integrated graphics, or the
 * CPU. Not to be confused with {@link Device}, which is a microphone. */
export interface ComputeDevice {
  name: string;
  description: string;
  kind: "cpu" | "gpu" | "integratedGpu" | "accelerator" | "unknown";
  memory: number | null;
}

/**
 * What is actually doing the speech recognition.
 *
 * `warning` is written in Rust and shown verbatim: a build made for one
 * vendor's card running on another's works perfectly and runs on the CPU, and
 * this is the only place in the interface that difference is visible.
 */
export interface Acceleration {
  accelerated: boolean;
  summary: string;
  warning: string | null;
  devices: ComputeDevice[];
}

export const acceleration = (): Promise<Acceleration> => invoke<Acceleration>("acceleration");

/** The languages worth offering. Whisper knows a hundred; nobody needs a
 * hundred-item menu, and `null` covers everything else by detecting. */
export const LANGUAGES: { code: string | null; label: string }[] = [
  { code: null, label: "Detect automatically" },
  { code: "en", label: "English" },
  { code: "ru", label: "Русский" },
  { code: "de", label: "Deutsch" },
  { code: "fr", label: "Français" },
  { code: "es", label: "Español" },
  { code: "it", label: "Italiano" },
  { code: "pt", label: "Português" },
  { code: "pl", label: "Polski" },
  { code: "uk", label: "Українська" },
  { code: "nl", label: "Nederlands" },
  { code: "tr", label: "Türkçe" },
  { code: "ja", label: "日本語" },
  { code: "zh", label: "中文" },
];
