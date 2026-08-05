//! The named commands the frontend is allowed to call.
//!
//! The frontend talks to Rust only through these and through events. No logic
//! is duplicated in TypeScript.

use klar_platform::{Binding, Permission, PermissionState};
use serde::Serialize;

#[derive(Serialize)]
pub struct AppVersion {
    version: &'static str,
    os: &'static str,
    arch: &'static str,
}

#[tauri::command]
pub fn app_version() -> AppVersion {
    AppVersion {
        version: klar_core::VERSION,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    }
}

/// The push-to-talk binding a fresh install starts on: Ctrl + Space on Windows,
/// ⌥ Space on macOS.
#[tauri::command]
pub fn default_hotkey() -> Binding {
    klar_platform::default_binding()
}

#[derive(Serialize)]
pub struct PermissionReport {
    permission: Permission,
    state: PermissionState,
}

/// The microphones the user could pick, default first.
#[tauri::command]
pub fn audio_devices() -> Result<Vec<klar_core::audio::Device>, String> {
    klar_core::audio::capture_devices().map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct ModelStatus {
    #[serde(flatten)]
    spec: &'static klar_core::model::ModelSpec,
    installed: bool,
    size: String,
}

/// The model catalogue and what is already on disk.
#[tauri::command]
pub fn models() -> Result<Vec<ModelStatus>, String> {
    let dir = klar_core::model::models_dir().ok_or("no app data directory")?;
    Ok(klar_core::model::CATALOGUE
        .iter()
        .map(|spec| ModelStatus {
            spec,
            installed: klar_core::model::is_present(&dir, spec),
            size: spec.human_size(),
        })
        .collect())
}

/// What the OS currently says about each permission onboarding walks through.
#[tauri::command]
pub fn permission_states() -> Vec<PermissionReport> {
    [Permission::Microphone, Permission::Accessibility]
        .into_iter()
        .map(|permission| PermissionReport {
            permission,
            state: klar_platform::permission_state(permission),
        })
        .collect()
}
