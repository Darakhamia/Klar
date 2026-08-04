//! Windows backend.
//!
//! Filled in at M2. The shape is fixed here so the rest of the app can be
//! written against it:
//!
//! - **Hotkey** — a `WH_KEYBOARD_LL` low-level keyboard hook on a dedicated
//!   thread with its own message pump. `RegisterHotKey` is not an option: it
//!   does not report key-up.
//! - **Injection** — `OpenClipboard`/`SetClipboardData` with the previous
//!   contents saved, then `SendInput` for Ctrl+V, then restore.
//! - **Elevation** — injecting into an elevated window from a non-elevated
//!   process fails silently. Compare the integrity level of the foreground
//!   window's process against ours and return [`PlatformError::ElevatedTarget`]
//!   before trying, so the user gets an error instead of a hang.

use crate::{
    Binding, Hotkey, HotkeyEvent, InjectionMethod, Permission, PermissionState, PlatformError,
    TextInjector,
};

pub fn hotkey() -> Box<dyn Hotkey> {
    Box::new(WindowsHotkey)
}

pub fn injector() -> Box<dyn TextInjector> {
    Box::new(WindowsInjector)
}

pub fn permission_state(permission: Permission) -> PermissionState {
    match permission {
        // Windows gates the microphone in Settings › Privacy, but a desktop
        // (non-packaged) app is not blocked by the toggle the way a packaged one
        // is; treat it as unknown until capture is real in M1 and the answer can
        // come from actually opening the device.
        Permission::Microphone => PermissionState::Unknown,
        // No Windows equivalent — the low-level hook and SendInput need no grant.
        Permission::Accessibility => PermissionState::NotApplicable,
    }
}

pub fn open_permission_settings(permission: Permission) -> Result<(), PlatformError> {
    match permission {
        Permission::Microphone => Err(PlatformError::NotImplemented(
            "ms-settings:privacy-microphone",
        )),
        Permission::Accessibility => Ok(()),
    }
}

struct WindowsHotkey;

impl Hotkey for WindowsHotkey {
    fn register(
        &mut self,
        _binding: &Binding,
        _on_event: Box<dyn FnMut(HotkeyEvent) + Send>,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented("WH_KEYBOARD_LL hook (M2)"))
    }

    fn unregister(&mut self) -> Result<(), PlatformError> {
        Ok(())
    }
}

struct WindowsInjector;

impl TextInjector for WindowsInjector {
    fn inject(&mut self, _text: &str) -> Result<InjectionMethod, PlatformError> {
        Err(PlatformError::NotImplemented("clipboard injection (M2)"))
    }
}
