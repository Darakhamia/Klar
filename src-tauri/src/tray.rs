//! The tray icon and its menu.
//!
//! Klar has no dock presence and no window open most of the time, so the tray
//! is where the app exists as far as the user is concerned.

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

/// Sent when the user asks for a particular pane, rather than whichever one the
/// settings window happened to be left on. The window is hidden rather than
/// closed, so it keeps its state for weeks.
pub const SECTION_EVENT: &str = "klar://section";

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id("klar")
        .icon(app.default_window_icon().cloned().ok_or_else(|| {
            tauri::Error::AssetNotFound("the bundled window icon is missing".into())
        })?)
        .tooltip("Klar")
        .menu(&menu(app, None)?)
        // Left click opens settings; the menu is on right click, which is what
        // Windows users expect from a tray icon.
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu)
        .build(app)?;

    Ok(())
}

/// The menu, with or without the update item.
///
/// Rebuilt whole rather than holding item handles and toggling them: muda has
/// no way to hide a menu item, only to disable one, and a permanently greyed
/// "Update to nothing" is worse than an item that appears when there is
/// something to say.
fn menu(app: &AppHandle, update: Option<&str>) -> tauri::Result<Menu<Wry>> {
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Klar", true, None::<&str>)?;

    let Some(version) = update else {
        return Menu::with_items(
            app,
            &[&settings, &PredefinedMenuItem::separator(app)?, &quit],
        );
    };

    let install = MenuItem::with_id(
        app,
        "update",
        format!("Update to {version}…"),
        true,
        None::<&str>,
    )?;

    // First, above its own separator: it is the only item here that is news,
    // and it goes away again once it has been acted on.
    Menu::with_items(
        app,
        &[
            &install,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "settings" => show_settings(app),
        // The ellipsis is not decoration: this opens the window rather than
        // installing. Installing restarts Klar, and doing that straight off a
        // tray click would take the hotkey out from under somebody mid-sentence.
        // General has the notes and the button.
        "update" => {
            show_section(app, "General");
            show_settings(app);
        }
        "quit" => app.exit(0),
        other => tracing::warn!(id = other, "unhandled tray menu item"),
    }
}

/// Say, in the places Klar is visible, that there is a newer version.
///
/// This used to set the tray tooltip and nothing else, on the reasoning that a
/// background app which interrupts to talk about itself is the thing Klar is
/// trying not to be. The reasoning was right and the result was useless: a
/// tooltip is read by somebody who is already hovering over the icon, wondering,
/// and nobody hovers over a tray icon wondering. The update reached nobody.
///
/// So: a menu item, which is where a tray app talks and costs nothing until it
/// is opened, and one notification, sent once per version by
/// [`super::updates::announce`] and never again for that version.
pub fn announce_update(app: &AppHandle, version: &str) {
    let Some(tray) = app.tray_by_id("klar") else {
        return;
    };

    match menu(app, Some(version)) {
        Ok(menu) => {
            if let Err(error) = tray.set_menu(Some(menu)) {
                tracing::warn!(%error, "could not put the update into the tray menu");
            }
        }
        Err(error) => tracing::warn!(%error, "could not rebuild the tray menu"),
    }

    if let Err(error) = tray.set_tooltip(Some(format!("Klar — {version} is available"))) {
        tracing::warn!(%error, "could not update the tray tooltip");
    }
}

/// Ask the settings window to show a particular pane.
///
/// Emitted before the window is shown so the pane has already changed by the
/// time it appears, rather than switching under the user's eyes.
pub fn show_section(app: &AppHandle, section: &str) {
    if let Err(error) = app.emit(SECTION_EVENT, section) {
        tracing::warn!(%error, section, "could not ask for a section");
    }
}

pub fn show_settings(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        tracing::warn!("no main window to show");
        return;
    };
    if let Err(error) = window.show().and_then(|()| window.set_focus()) {
        tracing::warn!(%error, "could not bring up the settings window");
    }
}
