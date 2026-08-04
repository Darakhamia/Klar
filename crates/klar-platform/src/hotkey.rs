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
