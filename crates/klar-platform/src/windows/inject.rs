//! Putting the finished text where the cursor is.
//!
//! Clipboard and Ctrl+V, because it is the only approach that handles Unicode,
//! emoji and a paragraph of text in one go. Synthesised keystrokes remain as a
//! fallback for applications that refuse a programmatic paste.
//!
//! The invariant that shapes this file: **the user's clipboard comes back**.
//! Every path out of [`WindowsInjector::inject`] restores the snapshot,
//! including the ones that failed.

use super::{clipboard, elevation};
use crate::{InjectionMethod, PlatformError, TextInjector};
use std::time::Duration;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};

/// 'V'. There is no VK constant for the letter keys; they are their ASCII codes.
const VK_V: VIRTUAL_KEY = VIRTUAL_KEY(0x56);

/// How long to leave our text on the clipboard before putting the user's back.
///
/// The paste is asynchronous: `SendInput` returns as soon as the events are
/// queued, and the target application reads the clipboard whenever it gets
/// round to the keystroke. Restore too early and the user gets their old
/// clipboard pasted instead of their dictation.
const PASTE_SETTLE: Duration = Duration::from_millis(150);

/// Above this, typing character by character is too slow to consider.
const KEYSTROKE_LIMIT: usize = 200;

pub struct WindowsInjector;

impl WindowsInjector {
    pub const fn new() -> Self {
        Self
    }
}

impl TextInjector for WindowsInjector {
    fn inject(&mut self, text: &str) -> Result<InjectionMethod, PlatformError> {
        self.inject_using(text, InjectionMethod::Clipboard)
    }

    fn inject_using(
        &mut self,
        text: &str,
        method: InjectionMethod,
    ) -> Result<InjectionMethod, PlatformError> {
        if text.is_empty() {
            return Ok(method);
        }

        // Ask before acting: a paste into an elevated window is dropped without
        // an error, and the user would see nothing happen at all.
        elevation::check_foreground()?;

        if method == InjectionMethod::Keystrokes {
            // No clipboard involved, so nothing to save or restore.
            type_text(text)?;
            return Ok(InjectionMethod::Keystrokes);
        }

        let snapshot = clipboard::snapshot()?;
        if !snapshot.is_complete() {
            tracing::warn!(
                formats = ?snapshot.skipped_formats(),
                "some clipboard contents cannot be restored after this dictation"
            );
        }

        // From here on, every exit restores.
        let outcome = paste(text);

        match outcome {
            Ok(()) => {
                // Give the target time to read the clipboard before taking it
                // back. Off the caller's thread: the text is already on its way,
                // and the 50 ms injection budget covers reaching the app, not
                // tidying up afterwards.
                std::thread::Builder::new()
                    .name("klar-clipboard-restore".into())
                    .spawn(move || {
                        std::thread::sleep(PASTE_SETTLE);
                        if let Err(error) = snapshot.restore() {
                            tracing::error!(%error, "could not restore the clipboard");
                        }
                    })
                    .map_err(|e| PlatformError::Clipboard(e.to_string()))?;
                Ok(InjectionMethod::Clipboard)
            }
            Err(error) => {
                // Restore immediately and synchronously — nothing was pasted,
                // so there is nothing to wait for.
                if let Err(restore_error) = snapshot.restore() {
                    tracing::error!(%restore_error, "could not restore the clipboard after a failed paste");
                }
                Err(error)
            }
        }
    }
}

/// Put the text on the clipboard and synthesise the paste chord.
fn paste(text: &str) -> Result<(), PlatformError> {
    clipboard::set_text(text)?;

    // The user was holding a hotkey a moment ago. Any modifier still down would
    // turn Ctrl+V into Ctrl+Shift+V or worse, so clear them first.
    release_modifiers();

    let events = [
        key(VK_CONTROL, false),
        key(VK_V, false),
        key(VK_V, true),
        key(VK_CONTROL, true),
    ];
    send(&events)
}

/// Type the text one character at a time.
///
/// The fallback for applications that block a programmatic paste. Slower by
/// orders of magnitude and used only when asked for, but it does not touch the
/// clipboard at all.
pub fn type_text(text: &str) -> Result<(), PlatformError> {
    if text.chars().count() > KEYSTROKE_LIMIT {
        return Err(PlatformError::Os(format!(
            "refusing to type {} characters one at a time; use the clipboard path",
            text.chars().count()
        )));
    }

    // KEYEVENTF_UNICODE takes UTF-16, so anything outside the basic plane —
    // emoji, most of them — arrives as two events.
    let mut events = Vec::with_capacity(text.len() * 2);
    for unit in text.encode_utf16() {
        events.push(unicode_key(unit, false));
        events.push(unicode_key(unit, true));
    }
    send(&events)
}

fn release_modifiers() {
    let events: Vec<INPUT> = [VK_SHIFT, VK_MENU, VK_LWIN]
        .into_iter()
        .map(|vk| key(vk, true))
        .collect();
    if let Err(error) = send(&events) {
        tracing::warn!(%error, "could not clear held modifiers before pasting");
    }
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn unicode_key(unit: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: if up {
                    KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                } else {
                    KEYEVENTF_UNICODE
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(events: &[INPUT]) -> Result<(), PlatformError> {
    if events.is_empty() {
        return Ok(());
    }

    // SAFETY: `events` is a valid slice of INPUT and the size argument matches
    // the type the API expects.
    let sent = unsafe { SendInput(events, i32::try_from(size_of::<INPUT>()).unwrap_or(0)) };

    if sent as usize == events.len() {
        Ok(())
    } else {
        // A short count means the input was blocked — most often UIPI, which
        // the elevation check should already have caught.
        Err(PlatformError::Os(format!(
            "SendInput delivered {sent} of {} events; the target may be elevated or blocking input",
            events.len()
        )))
    }
}
