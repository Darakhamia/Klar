//! The first-run window.
//!
//! Two steps on Windows, three on macOS — the accessibility grant has no
//! Windows equivalent, so that step is simply not there rather than shown and
//! skipped. The page decides which steps it has; this module only owns the
//! window and the handover at the end.

use tauri::{AppHandle, Manager};

pub const LABEL: &str = "onboarding";

/// Bring the first-run window up.
pub fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        tracing::error!("no onboarding window in the configuration");
        return;
    };
    if let Err(error) = window.show().and_then(|()| window.set_focus()) {
        tracing::error!(%error, "could not show the onboarding window");
    }
}

/// Onboarding is over: put the window away and leave the settings window up, so
/// the app the user just set up is the thing in front of them.
pub fn finish(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL)
        && let Err(error) = window.close()
    {
        tracing::warn!(%error, "could not close the onboarding window");
    }
    crate::tray::show_settings(app);
}
