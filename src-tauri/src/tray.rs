//! The tray icon and its menu.
//!
//! Klar has no dock presence and no window open most of the time, so the tray
//! is where the app exists as far as the user is concerned.

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Klar", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(app, &[&settings, &separator, &quit])?;

    TrayIconBuilder::with_id("klar")
        .icon(app.default_window_icon().cloned().ok_or_else(|| {
            tauri::Error::AssetNotFound("the bundled window icon is missing".into())
        })?)
        .tooltip("Klar")
        .menu(&menu)
        // Left click opens settings; the menu is on right click, which is what
        // Windows users expect from a tray icon.
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu)
        .build(app)?;

    Ok(())
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "settings" => show_settings(app),
        "quit" => app.exit(0),
        other => tracing::warn!(id = other, "unhandled tray menu item"),
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
