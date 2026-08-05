//! The Tauri wrapper: commands, tray, windows, IPC. No pipeline logic — that
//! all lives in `klar-core`, so it stays testable from `klar-cli`.

mod commands;
mod engine;
mod logging;
mod overlay;
mod tray;

use engine::{Engine, EngineConfig, UiEvent};
use klar_core::State;
use tauri::{AppHandle, Listener, Manager};

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
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            tray::build(&handle)?;
            overlay::prepare(&handle)?;
            follow_state(&handle);

            let engine = Engine::start(handle.clone(), EngineConfig::default());
            app.manage(engine);

            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the settings window hides it. Klar is a background app;
            // the tray is how it is quit.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            tracing::error!(%error, "tauri failed to start");
            std::process::exit(1);
        });
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
