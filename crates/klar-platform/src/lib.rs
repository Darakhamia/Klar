//! Everything that differs between macOS and Windows, behind two traits.
//!
//! Nothing above this crate branches on the operating system. Real backends
//! land in M2; at M0 each platform module exists with its types wired up and
//! its methods returning [`PlatformError::NotImplemented`], so the shape of the
//! boundary is fixed before any OS API is touched.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod hotkey;
pub mod inject;
pub mod permissions;

pub use hotkey::{Binding, Hotkey, HotkeyEvent, Key, Modifier};
pub use inject::{InjectionMethod, TextInjector};
pub use permissions::{Permission, PermissionState};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as backend;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as backend;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod unsupported;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use unsupported as backend;

/// Anything the operating system can refuse us.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    /// The user has not granted a permission this operation needs. The UI turns
    /// this into a link to the right system settings pane.
    #[error("permission not granted: {0}")]
    PermissionDenied(Permission),

    /// Windows: a non-elevated process cannot inject into an elevated window.
    /// Detected and reported rather than left to look like a hang.
    #[error("the focused window is elevated; Klar cannot type into it")]
    ElevatedTarget,

    /// The clipboard could not be read, written or restored.
    #[error("clipboard unavailable: {0}")]
    Clipboard(String),

    /// The hook could not be installed, or the binding is already taken.
    #[error("hotkey unavailable: {0}")]
    Hotkey(String),

    /// The OS call failed for a reason worth reporting verbatim.
    #[error("{0}")]
    Os(String),

    /// Reached on a platform, or in a milestone, where this is not built yet.
    #[error("not implemented on this platform yet: {0}")]
    NotImplemented(&'static str),
}

/// The default push-to-talk binding for the platform this build targets.
pub fn default_binding() -> Binding {
    if cfg!(target_os = "macos") {
        Binding::macos_default()
    } else {
        Binding::windows_default()
    }
}

/// Construct the platform's hotkey backend.
pub fn hotkey() -> Box<dyn Hotkey> {
    backend::hotkey()
}

/// Construct the platform's text injector.
pub fn injector() -> Box<dyn TextInjector> {
    backend::injector()
}

/// Ask the OS whether `permission` has been granted.
pub fn permission_state(permission: Permission) -> PermissionState {
    backend::permission_state(permission)
}

/// Open the system settings pane where the user grants `permission`.
pub fn open_permission_settings(permission: Permission) -> Result<(), PlatformError> {
    backend::open_permission_settings(permission)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_binding_matches_the_platform() {
        let expected = if cfg!(target_os = "macos") {
            Binding::macos_default()
        } else {
            Binding::windows_default()
        };
        assert_eq!(default_binding(), expected);
    }
}
