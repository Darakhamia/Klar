//! The overlay window.
//!
//! Borderless, always on top, and click-through: it sits over whatever the user
//! is working in and must never take focus or intercept a click. It renders
//! purely from the engine's events — see [`crate::engine`].

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow};

pub const LABEL: &str = "overlay";

/// 260×64 at rest, from the design. It grows to show recognised text and never
/// past 360, so the window is created at the maximum and the page draws inside.
const WIDTH: f64 = 380.0;
const HEIGHT: f64 = 96.0;

/// How far above the bottom of the screen to sit, as a fraction of its height.
/// Low enough to stay out of the way, high enough to clear a taskbar.
const BOTTOM_MARGIN: f64 = 0.12;

/// Place the overlay at the bottom centre of the screen holding the cursor.
pub fn position(window: &WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = window.primary_monitor()? else {
        return Ok(());
    };

    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let origin = monitor.position().to_logical::<f64>(scale);

    window.set_size(LogicalSize::new(WIDTH, HEIGHT))?;
    window.set_position(LogicalPosition::new(
        origin.x + (size.width - WIDTH) / 2.0,
        origin.y + size.height - HEIGHT - size.height * BOTTOM_MARGIN,
    ))
}

/// Prepare the overlay: position it and make it transparent to the mouse.
///
/// Click-through is set once and left on. The overlay has nothing to click —
/// dismissing an error happens from the tray or by starting another dictation —
/// and a window that swallows clicks over someone's editor is worse than no
/// overlay at all.
pub fn prepare(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(LABEL) else {
        tracing::warn!("no overlay window in the configuration");
        return Ok(());
    };

    position(&window)?;
    window.set_ignore_cursor_events(true)?;
    Ok(())
}

pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        // Re-position on every appearance: the user may have moved to another
        // monitor since the last dictation.
        if let Err(error) = position(&window) {
            tracing::warn!(%error, "could not place the overlay");
        }
        if let Err(error) = window.show() {
            tracing::warn!(%error, "could not show the overlay");
        }
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL)
        && let Err(error) = window.hide()
    {
        tracing::warn!(%error, "could not hide the overlay");
    }
}
