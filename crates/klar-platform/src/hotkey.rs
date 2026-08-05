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

/// The non-modifier key that completes a binding.
///
/// The hook only swallows this key while the binding's modifiers are also
/// held, so binding a letter costs nothing the rest of the time. Bound *bare*
/// is the dangerous case, which is what [`Self::is_bindable_alone`] is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Key {
    Space,
    /// macOS only, and only reachable through an `NSEvent` global monitor —
    /// `fn` does not appear in a normal event tap.
    Fn,
    /// F1 to F12. Anything higher is not on the keyboards people have.
    Function(u8),
    /// A letter or a digit, held lowercase: `a`–`z`, `0`–`9`.
    Character(char),
}

/// The highest function key Klar will bind.
pub const HIGHEST_FUNCTION_KEY: u8 = 12;

impl Key {
    /// A letter or digit as a [`Key`], or `None` for anything else.
    pub fn from_character(character: char) -> Option<Self> {
        let lowered = character.to_ascii_lowercase();
        lowered
            .is_ascii_alphanumeric()
            .then_some(Self::Character(lowered))
    }

    /// True for keys that mean nothing on their own, and so may be bound
    /// without a modifier.
    ///
    /// Space and the character keys may not: bound bare, the hook would
    /// swallow every one the user typed, everywhere, for as long as Klar runs.
    pub const fn is_bindable_alone(self) -> bool {
        matches!(self, Self::Function(_) | Self::Fn)
    }

    /// Whether this is a key at all, as opposed to a number outside the range
    /// or a character that is not a letter or a digit.
    pub const fn is_valid(self) -> bool {
        match self {
            Self::Space | Self::Fn => true,
            Self::Function(number) => number >= 1 && number <= HIGHEST_FUNCTION_KEY,
            Self::Character(character) => character.is_ascii_alphanumeric(),
        }
    }
}

/// Why a chord the user pressed cannot be used as a binding.
///
/// Returned as a value rather than logged: this is a sentence the settings
/// window shows, and every case has a different fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BadBinding {
    #[error("Klar binds a letter, a digit, Space or F1–F12. That key is none of them.")]
    UnsupportedKey,

    #[error(
        "That key needs a modifier — Ctrl, Alt, Shift or Win. \
         Bound on its own, Klar would swallow it everywhere you type."
    )]
    NeedsModifier,

    #[error("Nothing was pressed.")]
    TimedOut,

    #[error("Cancelled — the hotkey is unchanged.")]
    Cancelled,
}

impl Binding {
    /// Whether this chord is safe to install as push-to-talk.
    pub fn check(&self) -> Result<(), BadBinding> {
        if !self.key.is_valid() {
            return Err(BadBinding::UnsupportedKey);
        }
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
