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
/// Any key will do. The hook only swallows it while the binding's modifiers are
/// also held, so binding one costs nothing the rest of the time. Bound *bare*
/// is the case that needs care, which is what [`Self::is_bindable_alone`] is
/// for.
///
/// The named variants exist so the common bindings read as themselves in the
/// settings file and on screen. Everything else is carried by the platform's
/// own key code rather than dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Key {
    Space,
    /// macOS only, and only reachable through an `NSEvent` global monitor —
    /// `fn` does not appear in a normal event tap.
    Fn,
    /// F1 to F12. Anything higher is not on the keyboards people have.
    Function(u8),
    /// A key that types something: `a`, `7`, `/`, `,`. Held as the character it
    /// produces unshifted, which is what makes it recognisable in the settings
    /// file and on the key cap.
    Character(char),
    /// A key that types nothing and has no portable name — Tab, Insert, an
    /// arrow — carried by the platform's own key code.
    ///
    /// Windows virtual-key codes today. macOS will need its own mapping when it
    /// gets a backend, and a settings file does not travel between the two.
    /// Storing the code rather than a name keeps every key bindable without a
    /// table of every key that exists.
    Code(u32),
}

/// The highest function key Klar will bind.
pub const HIGHEST_FUNCTION_KEY: u8 = 12;

impl Key {
    /// The key a browser `KeyboardEvent.code` names, or `None` for one this
    /// layer does not know — the platform backend gets those.
    ///
    /// The settings window reads the chord from its own keyboard events rather
    /// than from the push-to-talk hook. It has to: the hook is not handed
    /// keystrokes while Klar's own window has focus, which is exactly the
    /// moment somebody is choosing a hotkey. A window that has focus is handed
    /// its own key events by definition, so that is where the chord comes from.
    ///
    /// `code` names the physical key rather than the character it produces, so
    /// this mapping does not move when the layout does.
    pub fn from_browser_code(code: &str) -> Option<Self> {
        if code == "Space" {
            return Some(Self::Space);
        }

        if let Some(number) = code.strip_prefix('F')
            && let Ok(number) = number.parse::<u8>()
            && (1..=HIGHEST_FUNCTION_KEY).contains(&number)
        {
            return Some(Self::Function(number));
        }

        // `KeyA` and `Digit1` name the key at that position on a US layout,
        // which is the same physical key everywhere.
        for prefix in ["Key", "Digit", "Numpad"] {
            if let Some(rest) = code.strip_prefix(prefix)
                && let Some(character) = one_character(rest)
            {
                return Some(Self::Character(character));
            }
        }

        let punctuation = match code {
            "Minus" => '-',
            "Equal" => '=',
            "BracketLeft" => '[',
            "BracketRight" => ']',
            "Backslash" => '\\',
            "Semicolon" => ';',
            "Quote" => '\'',
            "Comma" => ',',
            "Period" => '.',
            "Slash" => '/',
            "Backquote" => '`',
            _ => return None,
        };
        Some(Self::Character(punctuation))
    }

    /// True for keys that may be bound without a modifier.
    ///
    /// The rule is what the key does when Klar is not looking: a key that types
    /// a character must not be bound bare, because the hook would swallow every
    /// one the user typed, everywhere, for as long as Klar runs. A key that
    /// types nothing — a function key, Insert — costs nothing to hold.
    pub const fn is_bindable_alone(self) -> bool {
        matches!(self, Self::Function(_) | Self::Fn | Self::Code(_))
    }

    /// Whether this is a key at all, as opposed to a function number outside
    /// the range that exists.
    pub const fn is_valid(self) -> bool {
        match self {
            Self::Space | Self::Fn | Self::Character(_) | Self::Code(_) => true,
            Self::Function(number) => number >= 1 && number <= HIGHEST_FUNCTION_KEY,
        }
    }
}

/// Why a chord the user pressed cannot be used as a binding.
///
/// Returned as a value rather than logged: this is a sentence the settings
/// window shows, and every case has a different fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BadBinding {
    /// Reachable only for a key the platform itself will not report — `fn` on
    /// Windows, or a function number past what exists.
    #[error("That key cannot be used for push-to-talk on this system.")]
    UnsupportedKey,

    #[error(
        "That key types something, so it needs a modifier — Ctrl, Alt, Shift or Win. \
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

/// The single ASCII alphanumeric in `rest`, lowercased, or `None` if it is
/// anything else. `KeyA` gives `a`; `Enter` gives nothing.
fn one_character(rest: &str) -> Option<char> {
    let mut characters = rest.chars();
    let first = characters.next()?;
    if characters.next().is_some() || !first.is_ascii_alphanumeric() {
        return None;
    }
    Some(first.to_ascii_lowercase())
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

/// The key a browser `KeyboardEvent.code` names, including the ones with no
/// portable spelling — Tab, Insert, an arrow — which the platform supplies.
pub fn key_from_browser_code(code: &str) -> Option<Key> {
    Key::from_browser_code(code).or_else(|| crate::backend::key_from_browser_code(code))
}

/// Stop the push-to-talk hook acting, without unregistering it.
///
/// For the moment the settings window is reading a new chord: the hook would
/// otherwise swallow the current hotkey and start a dictation instead of
/// letting the window see the key.
pub fn suspend(suspended: bool) {
    crate::backend::suspend(suspended);
}
