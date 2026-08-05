//! The Tauri wrapper: commands, tray, windows, IPC. No pipeline logic — that
//! all lives in `klar-core`, so it stays testable from `klar-cli`.

mod commands;
mod downloads;
mod engine;
mod logging;
mod mic;
mod onboarding;
mod overlay;
mod settings;
mod tray;

use engine::{Engine, EngineConfig, UiEvent};
use klar_core::State;
use parking_lot::Mutex;
use settings::Settings;
use tauri::{AppHandle, Listener, Manager};

/// The running engine, replaced whenever settings change.
struct Running(Mutex<Option<Engine>>);

/// Entry point shared by the desktop binary and (eventually) any other host.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _log_guard = logging::init();
    tracing::info!(
        version = klar_core::VERSION,
        os = std::env::consts::OS,
        "klar starting"
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::default_hotkey,
            commands::permission_states,
            commands::audio_devices,
            commands::models,
            commands::settings_get,
            commands::settings_set,
            commands::engine_restart,
            commands::open_permission_settings,
            commands::model_download,
            commands::mic_test_start,
            commands::mic_test_stop,
            commands::hotkey_capture,
            commands::onboarding_finish,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            tray::build(&handle)?;
            overlay::prepare(&handle)?;
            follow_state(&handle);

            app.manage(Running(Mutex::new(None)));
            app.manage(commands::Microphone::default());
            app.manage(downloads::Active::default());

            let settings = Settings::load();
            restart_engine(&handle, &settings);

            // Both windows start hidden so a fresh install never flashes the
            // settings window behind onboarding.
            if settings.onboarded {
                tray::show_settings(&handle);
            } else {
                onboarding::show(&handle);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                match window.label() {
                    // Closing the settings window hides it. Klar is a
                    // background app; the tray is how it is quit.
                    "main" => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    // Onboarding closed without finishing. Let it go, but put
                    // the settings window up rather than leaving the user with
                    // an app that is running and has nothing on screen.
                    onboarding::LABEL => tray::show_settings(&window.app_handle().clone()),
                    _ => {}
                }
            }
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            tracing::error!(%error, "tauri failed to start");
            std::process::exit(1);
        });
}

/// Stop whatever is running and start again with these settings.
///
/// Replacing rather than reconfiguring: the engine holds a loaded model and a
/// registered keyboard hook, and most of these settings decide what those are.
pub fn restart_engine(app: &AppHandle, settings: &Settings) {
    let Some(running) = app.try_state::<Running>() else {
        tracing::error!("the engine slot is missing; settings will not take effect");
        return;
    };

    let mut slot = running.0.lock();
    if let Some(previous) = slot.take() {
        previous.stop();
    }
    *slot = Some(Engine::start(app.clone(), EngineConfig::from(settings)));
}

/// Show and hide the overlay in step with the pipeline.
///
/// Done in Rust rather than from the page: a window that shows itself has to be
/// running to do it, and the overlay is hidden most of the time.
fn follow_state(app: &AppHandle) {
    let handle = app.clone();
    app.listen(engine::EVENT, move |event| {
        let Ok(parsed) = serde_json::from_str::<UiEvent>(event.payload()) else {
            return;
        };
        match parsed {
            UiEvent::State { state: State::Idle } => overlay::hide(&handle),
            UiEvent::State { state: _ } => overlay::show(&handle),
            _ => {}
        }
    });
}
