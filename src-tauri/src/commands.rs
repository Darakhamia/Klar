//! The named commands the frontend is allowed to call.
//!
//! The frontend talks to Rust only through these and through events. No logic
//! is duplicated in TypeScript.

use crate::downloads::{self, Active};
use crate::mic::MicTest;
use crate::settings::Settings;
use klar_platform::{Binding, Permission, PermissionState};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

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

/// Stop or resume the push-to-talk hook while the window reads a chord.
///
/// The window captures the key itself, from its own keyboard events. The hook
/// has to stand down for that: it would otherwise swallow the current hotkey
/// and start a dictation instead of letting the window see the key.
#[tauri::command]
pub fn hotkey_suspend(suspended: bool) {
    tracing::info!(suspended, "push-to-talk hook suspended for a rebind");
    klar_platform::suspend(suspended);
}

/// Bind the chord the window read, and answer with what it means.
///
/// `code` is a browser `KeyboardEvent.code` — the physical key, not the
/// character it produces — and the platform layer decides what key that is.
/// The window reports what it saw; nothing about the keyboard is decided in
/// TypeScript.
///
/// The error is the refusal written for a person, and is shown as it is.
#[tauri::command]
pub fn hotkey_set(
    app: AppHandle,
    code: String,
    modifiers: Vec<klar_platform::Modifier>,
) -> Result<Binding, String> {
    let key = klar_platform::key_from_browser_code(&code).ok_or_else(|| {
        tracing::info!(code, "rebinding: unknown key");
        klar_platform::BadBinding::UnsupportedKey.to_string()
    })?;

    let binding = Binding { modifiers, key };
    binding.check().map_err(|error| {
        tracing::info!(?binding, %error, "rebinding: refused");
        error.to_string()
    })?;

    tracing::info!(?binding, "rebinding: accepted");

    let mut settings = Settings::load();
    settings.hotkey = binding.clone();
    settings.save()?;

    // Off this thread so the window gets its answer now: restarting reloads the
    // model, which is about a second, and nothing about the reply depends on it.
    let handle = app.clone();
    std::thread::spawn(move || {
        crate::restart_engine(&handle, &settings);
    });

    Ok(binding)
}

/// Let a window write into the same log file the Rust side uses.
///
/// Not a general logging facility, and not for chatter. It exists because the
/// two halves of this app fail independently: Rust can report a command
/// succeeding while the window that asked never hears the answer, and until now
/// the window's side of that went to a devtools console nobody had open. One
/// log with both halves in it is the difference between a diagnosis and a
/// guess.
#[tauri::command]
pub fn ui_log(window: String, message: String) {
    tracing::info!(window = %window, "ui: {message}");
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

/// What the local model server has, if it is running.
///
/// `models` is empty when it is not, which is the same answer the settings
/// window needs either way: there is nothing to choose from.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolishStatus {
    reachable: bool,
    endpoint: String,
    models: Vec<String>,
}

#[tauri::command]
pub async fn polish_status() -> PolishStatus {
    let settings = Settings::load();
    let endpoint = settings.polish_endpoint.clone();

    let config = klar_core::polish::OllamaConfig {
        endpoint: endpoint.clone(),
        model: settings.polish_model.clone(),
        // Only a listing; the dictation budget has nothing to do with it, and a
        // server that is starting up deserves longer than 400 ms to say hello.
        budget: std::time::Duration::from_secs(3),
        ..klar_core::polish::OllamaConfig::default()
    };

    let models = match klar_core::polish::Ollama::new(config) {
        Ok(ollama) => ollama.models().await.unwrap_or_default(),
        Err(error) => {
            tracing::warn!(%error, "could not build the polish client");
            Vec::new()
        }
    };

    PolishStatus {
        reachable: !models.is_empty(),
        endpoint,
        models,
    }
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
