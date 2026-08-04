//! Fallback backend for platforms Klar does not ship on.
//!
//! Linux is out of scope for v1, but the crate has to build there so the
//! pipeline can be developed and unit-tested off a target machine. Every call
//! fails loudly rather than pretending to work.

use crate::{
    Binding, Hotkey, HotkeyEvent, InjectionMethod, Permission, PermissionState, PlatformError,
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
}
