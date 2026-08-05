//! macOS backend.
//!
//! Windows ships first, so this stays a stub past M2. The constraints are
//! recorded now because they shape the trait:
//!
//! - **Hotkey** — `CGEventTap` on a run loop. If the user binds `fn`, that key
//!   never reaches an event tap and needs an `NSEvent` global monitor instead.
//! - **Accessibility** — required for both the tap and injection, and bound to
//!   the code signature. Ad-hoc signing with a stable identifier has to be set
//!   up before this is worked on, or the grant is lost on every rebuild.
//! - **Injection** — `NSPasteboard` save/set/restore around a synthesised
//!   Cmd+V via `CGEventPost`.

use crate::{
    Binding, Hotkey, HotkeyEvent, InjectionMethod, Key, Permission, PermissionState, PlatformError,
    TextInjector,
};

pub fn hotkey() -> Box<dyn Hotkey> {
    Box::new(MacHotkey)
}

pub fn injector() -> Box<dyn TextInjector> {
    Box::new(MacInjector)
}

pub fn permission_state(_permission: Permission) -> PermissionState {
    PermissionState::Unknown
}

pub fn open_permission_settings(_permission: Permission) -> Result<(), PlatformError> {
    Err(PlatformError::NotImplemented("macOS backend"))
}

pub fn copy_to_clipboard(_text: &str) -> Result<(), PlatformError> {
    Err(PlatformError::NotImplemented("NSPasteboard"))
}

/// macOS registers a login item through `SMAppService`, not a file the app
/// writes. Reporting `false` rather than erroring keeps the settings row honest
/// on a platform where nothing has been registered.
pub fn launch_at_login() -> Result<bool, PlatformError> {
    Ok(false)
}

pub fn set_launch_at_login(_on: bool) -> Result<(), PlatformError> {
    Err(PlatformError::NotImplemented("SMAppService login item"))
}

/// Nothing to stand down: there is no hook here to suspend.
pub fn suspend(_suspended: bool) {}

pub fn key_from_browser_code(_code: &str) -> Option<Key> {
    None
}

struct MacHotkey;

impl Hotkey for MacHotkey {
    fn register(
        &mut self,
        _binding: &Binding,
        _on_event: Box<dyn FnMut(HotkeyEvent) + Send>,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented("CGEventTap hook"))
    }

    fn unregister(&mut self) -> Result<(), PlatformError> {
        Ok(())
    }
}

struct MacInjector;

impl TextInjector for MacInjector {
    fn inject(&mut self, _text: &str) -> Result<InjectionMethod, PlatformError> {
        Err(PlatformError::NotImplemented("NSPasteboard injection"))
    }

    fn inject_and_keep(&mut self, _text: &str) -> Result<InjectionMethod, PlatformError> {
        Err(PlatformError::NotImplemented("NSPasteboard injection"))
    }

    fn inject_using(
        &mut self,
        _text: &str,
        _method: InjectionMethod,
    ) -> Result<InjectionMethod, PlatformError> {
        Err(PlatformError::NotImplemented("NSPasteboard injection"))
    }
}
