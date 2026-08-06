//! Windows backend.
//!
//! - **Hotkey** — a `WH_KEYBOARD_LL` hook on its own message-pumping thread.
//!   `RegisterHotKey` reports only the press, and the release is what ends a
//!   dictation. See [`hotkey`].
//! - **Injection** — the clipboard, saved and restored around a synthesised
//!   Ctrl+V, with per-character typing as a fallback. See [`inject`].
//! - **Elevation** — a paste into a higher-integrity window is dropped without
//!   an error, so it is detected up front. See [`elevation`].

mod autostart;
mod clipboard;
mod elevation;
mod hotkey;
mod inject;
mod keys;

pub use autostart::{launch_at_login, set_launch_at_login};
pub use elevation::foreground_app;
pub use hotkey::suspend;
pub use keys::key_from_browser_code;

use crate::{Hotkey, Permission, PermissionState, PlatformError, TextInjector};

pub fn hotkey() -> Box<dyn Hotkey> {
    Box::new(hotkey::WindowsHotkey::new())
}

pub fn copy_to_clipboard(text: &str) -> Result<(), PlatformError> {
    clipboard::set_text(text)
}

pub fn injector() -> Box<dyn TextInjector> {
    Box::new(inject::WindowsInjector::new())
}

pub fn permission_state(permission: Permission) -> PermissionState {
    match permission {
        // Windows gates the microphone in Settings › Privacy, but a desktop
        // (non-packaged) app is not blocked by the toggle the way a packaged
        // one is. The honest answer comes from opening the device, which is
        // what onboarding does.
        Permission::Microphone => PermissionState::Unknown,
        // No Windows equivalent — the low-level hook and SendInput need no
        // grant, only a non-elevated target.
        Permission::Accessibility => PermissionState::NotApplicable,
    }
}

pub fn open_permission_settings(permission: Permission) -> Result<(), PlatformError> {
    match permission {
        Permission::Microphone => open_url("ms-settings:privacy-microphone"),
        Permission::Accessibility => Ok(()),
    }
}

fn open_url(url: &str) -> Result<(), PlatformError> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::HSTRING;

    let target = HSTRING::from(url);
    let verb = HSTRING::from("open");

    // SAFETY: all pointers are into HSTRINGs alive for the call, and a null
    // parent window is valid for a settings URI.
    let result = unsafe { ShellExecuteW(None, &verb, &target, None, None, SW_SHOWNORMAL) };

    // ShellExecuteW returns a fake HINSTANCE; anything above 32 is success.
    if result.0 as usize > 32 {
        Ok(())
    } else {
        Err(PlatformError::Os(format!("could not open {url}")))
    }
}
