//! Push-to-talk.
//!
//! `tauri-plugin-global-shortcut` fires on press and gives no reliable key-up,
//! which is exactly the half we need. Hold-to-talk therefore goes through raw
//! OS hooks — `WH_KEYBOARD_LL` on Windows, `CGEventTap` on macOS — behind this
//! one trait, so nothing above it has to know which.

use crate::PlatformError;
use serde::{Deserialize, Serialize};

/// A key binding the user can hold. Modifiers are a set; `key` is the one
/// non-modifier key that completes the binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub modifiers: Vec<Modifier>,
    pub key: Key,
}

impl Binding {
    /// The Windows default from the design: Ctrl + Space.
    pub fn windows_default() -> Self {
        Self {
            modifiers: vec![Modifier::Control],
            key: Key::Space,
        }
    }

    /// The macOS default from the design: ⌥ Space.
    pub fn macos_default() -> Self {
        Self {
            modifiers: vec![Modifier::Alt],
            key: Key::Space,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Control,
    Alt,
    Shift,
    /// Command on macOS, the Windows key on Windows.
    Meta,
}

/// The non-modifier keys Klar will bind to.
///
/// Deliberately short. A push-to-talk key is held for seconds at a time and is
/// swallowed while Klar owns it, so the list is the keys people actually reach
/// for and nothing that would quietly break typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Key {
    Space,
    /// macOS only, and only reachable through an `NSEvent` global monitor —
    /// `fn` does not appear in a normal event tap.
    Fn,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl Key {
    /// True for keys that carry no meaning on their own, and so may be bound
    /// without a modifier.
    ///
    /// Space may not: binding it bare would swallow every space the user types,
    /// everywhere, for as long as Klar is running.
    pub const fn is_bindable_alone(self) -> bool {
        !matches!(self, Self::Space)
    }
}

/// Why a chord the user pressed cannot be used as a binding.
///
/// Returned as a value rather than logged: this is a sentence the settings
/// window shows, and every case has a different fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BadBinding {
    #[error("that key cannot be used for push-to-talk")]
    UnsupportedKey,

    #[error("Space needs a modifier — bound on its own it would swallow every space you type")]
    NeedsModifier,

    #[error("nothing was pressed")]
    TimedOut,

    #[error("cancelled")]
    Cancelled,
}

impl Binding {
    /// Whether this chord is safe to install as push-to-talk.
    pub fn check(&self) -> Result<(), BadBinding> {
        if self.modifiers.is_empty() && !self.key.is_bindable_alone() {
            return Err(BadBinding::NeedsModifier);
        }
        Ok(())
    }
}

/// Which edge of the hold fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}

/// A registered push-to-talk binding. Dropping it unregisters the hook.
pub trait Hotkey: Send {
    /// Start listening. `on_event` runs on the hook thread and must not block —
    /// a slow callback here stalls the whole system's input queue.
    ///
    /// It must also never panic: a panic on this path kills a background app the
    /// user cannot see.
    fn register(
        &mut self,
        binding: &Binding,
        on_event: Box<dyn FnMut(HotkeyEvent) + Send>,
    ) -> Result<(), PlatformError>;

    /// Stop listening. Idempotent.
    fn unregister(&mut self) -> Result<(), PlatformError>;
}

/// Wait for the user to press a chord, and report it instead of acting on it.
///
/// Blocks, so it belongs on a background thread. The key that completes the
/// chord is swallowed — the user is pressing it at a settings window, not at a
/// text field — and Escape cancels.
///
/// A push-to-talk hook must not be registered at the same time: two hooks
/// fighting over the same key would start a dictation while the user is trying
/// to rebind it. The caller stops the engine first.
pub fn capture(timeout: std::time::Duration) -> Result<Binding, PlatformError> {
    crate::backend::capture_binding(timeout)
}
