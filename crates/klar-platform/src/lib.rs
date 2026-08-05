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

pub use hotkey::{
    BadBinding, Binding, HIGHEST_FUNCTION_KEY, Hotkey, HotkeyEvent, Key, Modifier,
    key_from_browser_code, suspend,
};
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

    /// The chord the user pressed cannot be used. Carried through as itself so
    /// the settings window can show the specific reason.
    #[error("{0}")]
    Binding(#[from] BadBinding),

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

/// Put `text` on the clipboard and leave it there.
///
/// The opposite of what [`TextInjector`] does — injection borrows the clipboard
/// and gives it back. This is for the user asking for the text to be copied, in
/// which case replacing what was there is the whole point.
pub fn copy_to_clipboard(text: &str) -> Result<(), PlatformError> {
    backend::copy_to_clipboard(text)
}

/// Whether Klar is registered to start when the user logs in.
pub fn launch_at_login() -> Result<bool, PlatformError> {
    backend::launch_at_login()
}

/// Register or unregister Klar for launch at login.
///
/// Windows calls this "Start with Windows" and it is a value under the current
/// user's `Run` key — no elevation, and nothing outside the user's own hive.
pub fn set_launch_at_login(on: bool) -> Result<(), PlatformError> {
    backend::set_launch_at_login(on)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_may_not_be_bound_on_its_own() {
        let bare = Binding {
            modifiers: vec![],
            key: Key::Space,
        };
        assert_eq!(bare.check(), Err(BadBinding::NeedsModifier));

        let with_modifier = Binding {
            modifiers: vec![Modifier::Control],
            key: Key::Space,
        };
        assert_eq!(with_modifier.check(), Ok(()));
    }

    #[test]
    fn a_function_key_needs_no_modifier() {
        let bare = Binding {
            modifiers: vec![],
            key: Key::Function(9),
        };
        assert_eq!(bare.check(), Ok(()));
    }

    #[test]
    fn browser_codes_name_the_physical_key() {
        assert_eq!(Key::from_browser_code("Space"), Some(Key::Space));
        assert_eq!(Key::from_browser_code("F9"), Some(Key::Function(9)));
        assert_eq!(Key::from_browser_code("KeyD"), Some(Key::Character('d')));
        assert_eq!(Key::from_browser_code("Digit7"), Some(Key::Character('7')));
        // The keys the whole rebinding argument was actually about.
        assert_eq!(Key::from_browser_code("Slash"), Some(Key::Character('/')));
        assert_eq!(Key::from_browser_code("Comma"), Some(Key::Character(',')));
        assert_eq!(Key::from_browser_code("Quote"), Some(Key::Character('\'')));
        assert_eq!(
            Key::from_browser_code("Backslash"),
            Some(Key::Character('\\'))
        );
    }

    #[test]
    fn browser_codes_this_layer_does_not_know_are_left_to_the_platform() {
        // Named keys with no portable spelling. The backend maps these; on a
        // platform without one they are simply not bindable.
        assert_eq!(Key::from_browser_code("Insert"), None);
        assert_eq!(Key::from_browser_code("ArrowUp"), None);
        // Beyond any keyboard, and not a key at all.
        assert_eq!(Key::from_browser_code("F27"), None);
        assert_eq!(Key::from_browser_code(""), None);
        assert_eq!(Key::from_browser_code("KeyAB"), None);
    }

    #[test]
    fn both_defaults_are_bindings_klar_would_accept() {
        assert_eq!(Binding::windows_default().check(), Ok(()));
        assert_eq!(Binding::macos_default().check(), Ok(()));
    }

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
