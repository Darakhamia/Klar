//! The named commands the frontend is allowed to call.
//!
//! The frontend talks to Rust only through these and through events. No logic
//! is duplicated in TypeScript.

use crate::downloads::{self, Active};
use crate::mic::MicTest;
use crate::settings::{self, Settings};
use klar_platform::{Binding, Permission, PermissionState};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

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

/// Everything the user has chosen.
///
/// `launch_at_login` comes back from the OS rather than from the file: the user
/// can remove the startup entry from Task Manager without Klar running, and a
/// toggle still reading On would be a lie.
#[tauri::command]
pub fn settings_get() -> Settings {
    let mut settings = Settings::load();
    match klar_platform::launch_at_login() {
        Ok(on) => settings.launch_at_login = on,
        Err(error) => tracing::warn!(%error, "could not read the startup entry"),
    }
    settings
}

/// Replace the settings and restart the engine so they take effect.
///
/// Restarting rather than mutating: the engine holds a loaded model and a
/// registered hook, and half of these settings change which model that is.
#[tauri::command]
pub fn settings_set(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings.save()?;
    crate::restart_engine(&app, &settings);

    // Last, and reported separately: the rest of the settings have already
    // taken effect, and a registry that refused the write is worth a message
    // rather than throwing the whole save away.
    klar_platform::set_launch_at_login(settings.launch_at_login).map_err(|e| e.to_string())
}

/// Wait for the user to press a chord, and bind it.
///
/// The engine is stopped first: its hook owns the current binding, and pressing
/// it to rebind it would start a dictation instead. Blocks for as long as the
/// user takes, so it runs off the main thread and answers on `klar://hotkey`.
#[tauri::command]
pub fn hotkey_capture(app: AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        crate::stop_engine(&handle);

        let captured = klar_platform::capture(CAPTURE_TIMEOUT);

        let mut settings = Settings::load();
        let result = match captured {
            Ok(binding) => {
                settings.hotkey = binding.clone();
                match settings.save() {
                    Ok(()) => CaptureResult::Bound { binding },
                    Err(message) => CaptureResult::Refused { message },
                }
            }
            Err(error) => CaptureResult::Refused {
                message: error.to_string(),
            },
        };

        // The engine comes back either way: leaving the app without a hotkey
        // because the user pressed the wrong key would be the worst outcome
        // here by some distance.
        crate::restart_engine(&handle, &settings);
        settings::broadcast(&handle, &settings);

        if let Err(error) = handle.emit(CAPTURE_EVENT, &result) {
            tracing::warn!(%error, "could not report the captured hotkey");
        }
    });
}

/// How long the user has to press something before capture gives up. Long
/// enough to find a key, short enough that a forgotten capture ends itself.
const CAPTURE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub const CAPTURE_EVENT: &str = "klar://hotkey";

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
enum CaptureResult {
    Bound {
        binding: Binding,
    },
    /// Timed out, cancelled with Escape, or a chord Klar will not bind. The
    /// message is written for a person and shown as it is.
    Refused {
        message: String,
    },
}

/// Start the engine again on whatever is now on disk.
///
/// Onboarding calls this once the models have finished downloading: the engine
/// that started with the app gave up when it found nothing to load.
#[tauri::command]
pub fn engine_restart(app: AppHandle) {
    crate::restart_engine(&app, &Settings::load());
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

/// Open the system pane where the user grants `permission`.
#[tauri::command]
pub fn open_permission_settings(permission: Permission) -> Result<(), String> {
    klar_platform::open_permission_settings(permission).map_err(|e| e.to_string())
}

/// Start fetching a model. Progress arrives on `klar://model`.
#[tauri::command]
pub fn model_download(app: AppHandle, active: State<'_, Active>, id: String) -> Result<(), String> {
    let spec = klar_core::model::find(&id).ok_or_else(|| format!("unknown model {id}"))?;
    downloads::start(&app, &active, spec);
    Ok(())
}

/// The microphone test that is onboarding's first step. Emits levels on the
/// engine's own channel until [`mic_test_stop`].
#[tauri::command]
pub fn mic_test_start(app: AppHandle, running: State<'_, Microphone>) -> Result<(), String> {
    let device = Settings::load().microphone;
    let test = MicTest::start(app.clone(), device)?;
    // Assigning drops any previous test, which closes its stream.
    *running.0.lock() = Some(test);
    Ok(())
}

#[tauri::command]
pub fn mic_test_stop(running: State<'_, Microphone>) {
    running.0.lock().take();
}

/// The running microphone test, if any.
#[derive(Default)]
pub struct Microphone(pub Mutex<Option<MicTest>>);

/// First-run setup is finished: record it so the window does not come back, and
/// hand over to the settings window.
#[tauri::command]
pub fn onboarding_finish(app: AppHandle) -> Result<(), String> {
    if let Some(running) = app.try_state::<Microphone>() {
        running.0.lock().take();
    }

    let settings = Settings {
        onboarded: true,
        ..Settings::load()
    };
    settings.save()?;

    crate::onboarding::finish(&app);
    Ok(())
}
