//! The overlay window.
//!
//! Borderless, always on top, and click-through: it sits over whatever the user
//! is working in and must never take focus or intercept a click. It renders
//! purely from the engine's events — see [`crate::engine`].

use crate::settings::{OverlayPosition, Settings};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow};

pub const LABEL: &str = "overlay";

/// 260×64 at rest, from the design. It grows to show recognised text and never
/// past 360, so the window is created at the maximum and the page draws inside.
const WIDTH: f64 = 380.0;
const HEIGHT: f64 = 96.0;

/// How far from the top or bottom of the screen to sit, as a fraction of its
/// height. Far enough to stay out of the way, close enough to clear a taskbar.
const MARGIN: f64 = 0.12;

/// Put the overlay where the user asked for it, on the screen they are using.
///
/// The cursor decides which screen, not the primary monitor: somebody with a
/// laptop open next to a desktop display is looking at one of them, and it is
/// the one their mouse is on.
pub fn position(window: &WebviewWindow, where_to: OverlayPosition) -> tauri::Result<()> {
    let cursor = window.cursor_position().ok();
    let monitor = match cursor {
        Some(at) => window.monitor_from_point(at.x, at.y)?,
        None => window.primary_monitor()?,
    };
    let Some(monitor) = monitor else {
        return Ok(());
    };

    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let origin = monitor.position().to_logical::<f64>(scale);

    let centred = origin.x + (size.width - WIDTH) / 2.0;
    let (x, y) = match where_to {
        OverlayPosition::BottomCentre => (
            centred,
            origin.y + size.height - HEIGHT - size.height * MARGIN,
        ),
        OverlayPosition::TopCentre => (centred, origin.y + size.height * MARGIN),
        OverlayPosition::NearCursor => {
            let at = cursor.map(|at| at.to_logical::<f64>(scale));
            match at {
                // Below and left of the cursor, so it does not sit on top of
                // what is being typed into.
                Some(at) => (
                    (at.x - WIDTH / 2.0).clamp(origin.x, origin.x + size.width - WIDTH),
                    (at.y + 24.0).clamp(origin.y, origin.y + size.height - HEIGHT),
                ),
                // No cursor to be near. Falling back to the bottom beats
                // putting it in a corner.
                None => (
                    centred,
                    origin.y + size.height - HEIGHT - size.height * MARGIN,
                ),
            }
        }
    };

    window.set_size(LogicalSize::new(WIDTH, HEIGHT))?;
    window.set_position(LogicalPosition::new(x, y))
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

    position(&window, Settings::load().overlay_position)?;
    window.set_ignore_cursor_events(true)?;
    Ok(())
}

pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        // Re-positioned on every appearance rather than once: the user may
        // have moved to another monitor, or changed where they want it.
        if let Err(error) = position(&window, Settings::load().overlay_position) {
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
