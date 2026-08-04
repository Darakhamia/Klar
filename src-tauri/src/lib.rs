//! The Tauri wrapper: commands, tray, windows, IPC. No pipeline logic — that
//! all lives in `klar-core`, so it stays testable from `klar-cli`.

mod commands;
mod logging;

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
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            tracing::error!(%error, "tauri failed to start");
            std::process::exit(1);
        });
}
