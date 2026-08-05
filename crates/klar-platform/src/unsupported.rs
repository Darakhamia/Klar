//! Fallback backend for platforms Klar does not ship on.
//!
//! Linux is out of scope for v1, but the crate has to build there so the
//! pipeline can be developed and unit-tested off a target machine. Every call
//! fails loudly rather than pretending to work.

use crate::{
    Binding, Hotkey, HotkeyEvent, InjectionMethod, Key, Permission, PermissionState, PlatformError,
    TextInjector,
};

pub fn hotkey() -> Box<dyn Hotkey> {
    Box::new(NoHotkey)
}

pub fn injector() -> Box<dyn TextInjector> {
    Box::new(NoInjector)
}

pub fn permission_state(_permission: Permission) -> PermissionState {
    PermissionState::NotApplicable
}

pub fn open_permission_settings(_permission: Permission) -> Result<(), PlatformError> {
    Err(PlatformError::NotImplemented("permission settings"))
}

pub fn copy_to_clipboard(_text: &str) -> Result<(), PlatformError> {
    Err(PlatformError::NotImplemented("clipboard"))
}

pub fn launch_at_login() -> Result<bool, PlatformError> {
    Ok(false)
}

pub fn set_launch_at_login(_on: bool) -> Result<(), PlatformError> {
    Err(PlatformError::NotImplemented("launch at login"))
}

/// Nothing to stand down: there is no hook here to suspend.
pub fn suspend(_suspended: bool) {}

pub fn key_from_browser_code(_code: &str) -> Option<Key> {
    None
}

struct NoHotkey;

impl Hotkey for NoHotkey {
    fn register(
        &mut self,
        _binding: &Binding,
        _on_event: Box<dyn FnMut(HotkeyEvent) + Send>,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented("hotkey"))
    }

    fn unregister(&mut self) -> Result<(), PlatformError> {
        Ok(())
    }
}

struct NoInjector;

impl TextInjector for NoInjector {
    fn inject(&mut self, _text: &str) -> Result<InjectionMethod, PlatformError> {
        Err(PlatformError::NotImplemented("text injection"))
    }

    fn inject_and_keep(&mut self, _text: &str) -> Result<InjectionMethod, PlatformError> {
        Err(PlatformError::NotImplemented("text injection"))
    }

    fn inject_using(
        &mut self,
        _text: &str,
        _method: InjectionMethod,
    ) -> Result<InjectionMethod, PlatformError> {
        Err(PlatformError::NotImplemented("text injection"))
    }
}
