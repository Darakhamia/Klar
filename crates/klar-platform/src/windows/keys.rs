//! Mapping Klar's platform-independent [`Binding`] onto Windows virtual keys.

use crate::{Binding, HIGHEST_FUNCTION_KEY, Key, Modifier};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_F1, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    VK_SPACE, VkKeyScanW,
};

/// The virtual key for a character, on the layout in use right now.
fn key_of_character(character: char) -> Option<VIRTUAL_KEY> {
    // Letters and digits have fixed virtual keys — the ASCII code of the
    // uppercase character — and are asked for by name rather than by layout, so
    // a Cyrillic layout does not lose them.
    if character.is_ascii_alphanumeric() {
        return Some(VIRTUAL_KEY(character.to_ascii_uppercase() as u16));
    }

    let mut buffer = [0_u16; 2];
    let encoded = character.encode_utf16(&mut buffer);
    let unit = *encoded.first()?;

    // SAFETY: takes a UTF-16 code unit by value; no pointers.
    let scanned = unsafe { VkKeyScanW(unit) };
    if scanned == -1 {
        return None;
    }

    // The low byte is the virtual key; the high byte is the shift state needed
    // to type it, which is not part of the binding — the physical key is.
    Some(VIRTUAL_KEY((scanned as u16) & 0x00FF))
}

/// The virtual key a binding's trigger corresponds to, or `None` for keys that
/// do not exist on Windows or are outside the range Klar binds.
pub fn virtual_key(key: Key) -> Option<VIRTUAL_KEY> {
    match key {
        Key::Space => Some(VK_SPACE),
        // VK_F1 through VK_F24 are contiguous.
        Key::Function(number) if (1..=HIGHEST_FUNCTION_KEY).contains(&number) => {
            Some(VIRTUAL_KEY(VK_F1.0 + u16::from(number) - 1))
        }
        Key::Function(_) => None,
        Key::Character(character) => key_of_character(character),
        Key::Code(code) => u16::try_from(code).ok().map(VIRTUAL_KEY),
        // macOS only; there is no Windows equivalent to bind.
        Key::Fn => None,
    }
}

/// The keys a browser names but Klar has no portable spelling for, mapped to
/// the Windows virtual key behind them.
///
/// Deliberately short: the keys somebody might actually hold for push-to-talk,
/// not every key that exists. Anything missing is refused with a message rather
/// than bound to something wrong.
pub fn key_from_browser_code(code: &str) -> Option<Key> {
    let virtual_key: u32 = match code {
        "Tab" => 0x09,
        "CapsLock" => 0x14,
        "Insert" => 0x2D,
        "Delete" => 0x2E,
        "Home" => 0x24,
        "End" => 0x23,
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        "ArrowLeft" => 0x25,
        "ArrowUp" => 0x26,
        "ArrowRight" => 0x27,
        "ArrowDown" => 0x28,
        "NumpadMultiply" => 0x6A,
        "NumpadAdd" => 0x6B,
        "NumpadSubtract" => 0x6D,
        "NumpadDecimal" => 0x6E,
        "NumpadDivide" => 0x6F,
        _ => return None,
    };
    Some(Key::Code(virtual_key))
}

/// Whether every modifier the binding needs is physically down right now.
///
/// Read from the OS rather than tracked from the hook's own event stream:
/// a modifier pressed before Klar installed its hook would otherwise be
/// invisible, and the hotkey would refuse to fire until it was pressed again.
pub fn modifiers_held(binding: &Binding) -> bool {
    binding.modifiers.iter().all(|modifier| is_down(*modifier))
}

fn is_down(modifier: Modifier) -> bool {
    // `VK_CONTROL`, `VK_MENU` and `VK_SHIFT` cover both sides of the keyboard.
    // The Windows key does not have a combined code, so check each.
    match modifier {
        Modifier::Control => pressed(VK_CONTROL),
        Modifier::Alt => pressed(VK_MENU),
        Modifier::Shift => pressed(VK_SHIFT),
        Modifier::Meta => pressed(VK_LWIN) || pressed(VK_RWIN),
    }
}

fn pressed(key: VIRTUAL_KEY) -> bool {
    // The high bit means "down now"; the low bit means "pressed since the last
    // call", which is not what we want and is deliberately masked off.
    // SAFETY: reads keyboard state for a valid virtual key code; no pointers.
    let state = unsafe { GetAsyncKeyState(i32::from(key.0)) };
    (state as u16 & 0x8000) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_key_has_a_virtual_key() {
        let mut keys = vec![Key::Space];
        keys.extend((1..=HIGHEST_FUNCTION_KEY).map(Key::Function));
        keys.extend(
            "abcdefghijklmnopqrstuvwxyz0123456789"
                .chars()
                .map(Key::Character),
        );

        for key in keys {
            assert!(virtual_key(key).is_some(), "{key:?} has no VK");
        }
    }

    #[test]
    fn a_raw_code_maps_straight_back_to_its_virtual_key() {
        assert_eq!(virtual_key(Key::Code(0x2D)), Some(VIRTUAL_KEY(0x2D)));
        // Beyond a u16, so not a virtual key at all.
        assert_eq!(virtual_key(Key::Code(0xFFFF_0000)), None);
    }

    #[test]
    fn function_numbers_outside_the_keyboard_have_no_mapping() {
        assert_eq!(virtual_key(Key::Function(13)), None);
        assert_eq!(virtual_key(Key::Function(0)), None);
    }

    #[test]
    fn the_fn_key_is_not_bindable_on_windows() {
        assert!(virtual_key(Key::Fn).is_none());
    }

    #[test]
    fn the_windows_default_maps_to_space() {
        let binding = Binding::windows_default();
        assert_eq!(virtual_key(binding.key), Some(VK_SPACE));
    }

    #[test]
    fn a_binding_with_no_modifiers_is_always_satisfied() {
        let binding = Binding {
            modifiers: vec![],
            key: Key::Function(1),
        };
        assert!(modifiers_held(&binding));
    }
}
