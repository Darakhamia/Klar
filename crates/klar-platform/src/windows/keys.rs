//! Mapping Klar's platform-independent [`Binding`] onto Windows virtual keys.

use crate::{Binding, HIGHEST_FUNCTION_KEY, Key, Modifier};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MAPVK_VK_TO_CHAR, MapVirtualKeyW, VIRTUAL_KEY, VK_CONTROL, VK_F1,
    VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
    VK_SHIFT, VK_SPACE, VkKeyScanW,
};

/// The character a key produces unshifted, on the layout in use right now, or
/// `None` for keys that produce nothing — Tab, Insert, the arrows.
///
/// Layout-dependent by design: the point is that the binding reads as the key
/// cap the user pressed. A layout switch can move it, which is the same
/// bargain every other application makes with letter shortcuts.
fn character_of(code: u32) -> Option<char> {
    // SAFETY: no pointers; takes and returns integers.
    let mapped = unsafe { MapVirtualKeyW(code, MAPVK_VK_TO_CHAR) };

    // Zero means the key produces nothing. The top bit marks a dead key, which
    // is not something to bind either.
    if mapped == 0 || mapped & 0x8000_0000 != 0 {
        return None;
    }

    let character = char::from_u32(mapped & 0xFFFF)?;
    // Control codes come back for Tab, Enter and Backspace — those are keys
    // that do something rather than type something.
    (!character.is_control()).then(|| character.to_ascii_lowercase())
}

/// The virtual key for a character, on the layout in use right now.
fn key_of_character(character: char) -> Option<VIRTUAL_KEY> {
    // Letters and digits have fixed virtual keys — the ASCII code of the
    // uppercase character — and are asked for by name rather than by layout,
    // so a Cyrillic layout does not lose them.
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

/// The [`Key`] a virtual key code corresponds to. The inverse of
/// [`virtual_key`], used while capturing a chord.
///
/// Never `None`: every key is bindable, and one with no name of its own is
/// carried by its code rather than refused.
pub fn key_from_virtual(code: u32) -> Key {
    if code == u32::from(VK_SPACE.0) {
        return Key::Space;
    }

    let first_function = u32::from(VK_F1.0);
    if (first_function..first_function + u32::from(HIGHEST_FUNCTION_KEY)).contains(&code)
        && let Ok(offset) = u8::try_from(code - first_function)
    {
        return Key::Function(offset + 1);
    }

    match character_of(code) {
        Some(character) => Key::Character(character),
        None => Key::Code(code),
    }
}

/// True for the modifier keys themselves, which complete no chord on their own.
pub fn is_modifier_key(code: u32) -> bool {
    [
        VK_CONTROL,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_MENU,
        VK_LMENU,
        VK_RMENU,
        VK_SHIFT,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_LWIN,
        VK_RWIN,
    ]
    .iter()
    .any(|vk| u32::from(vk.0) == code)
}

/// The modifiers physically down right now, as a bitmask.
///
/// A mask rather than a `Vec` because this is read inside the hook callback,
/// which runs in the system's input path and does not allocate.
pub fn held_mask() -> u8 {
    let mut mask = 0;
    for (bit, modifier) in ORDER.iter().enumerate() {
        if is_down(*modifier) {
            mask |= 1 << bit;
        }
    }
    mask
}

/// Unpack what [`held_mask`] recorded. Order is fixed so a binding always
/// renders the same way.
pub fn modifiers_from_mask(mask: u8) -> Vec<Modifier> {
    ORDER
        .iter()
        .enumerate()
        .filter(|(bit, _)| mask & (1 << bit) != 0)
        .map(|(_, modifier)| *modifier)
        .collect()
}

/// The order modifiers are written in, everywhere.
const ORDER: [Modifier; 4] = [
    Modifier::Control,
    Modifier::Alt,
    Modifier::Shift,
    Modifier::Meta,
];

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
    fn virtual_keys_round_trip_back_to_their_key() {
        let keys = [
            Key::Space,
            Key::Function(1),
            Key::Function(9),
            Key::Function(12),
            Key::Character('a'),
            Key::Character('z'),
            Key::Character('0'),
            Key::Character('7'),
        ];
        for key in keys {
            let vk = virtual_key(key).expect("bindable");
            assert_eq!(key_from_virtual(u32::from(vk.0)), key);
        }
    }

    #[test]
    fn a_captured_letter_comes_back_lowercase() {
        // Windows reports the uppercase code for a letter key whether or not
        // Shift is held; the binding stores one spelling so it renders the same
        // way every time.
        assert_eq!(key_from_virtual(0x44), Key::Character('d'));
    }

    #[test]
    fn a_key_with_no_name_is_carried_by_its_code_rather_than_refused() {
        // Punctuation depends on the layout, so what it maps to is not
        // asserted — only that it is never thrown away.
        for code in [0xBF, 0xBC, 0xBE, 0xDC, 0xDE, 0x09, 0x2D, 0x25] {
            let key = key_from_virtual(code);
            assert!(
                matches!(key, Key::Character(_) | Key::Code(_)),
                "{code:#04x} came back as {key:?}"
            );
            assert!(key.is_valid(), "{key:?} should be bindable");
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

        assert!(is_modifier_key(u32::from(VK_LCONTROL.0)));
        assert!(!is_modifier_key(u32::from(VK_SPACE.0)));
    }

    #[test]
    fn a_mask_unpacks_to_the_modifiers_that_made_it() {
        assert_eq!(modifiers_from_mask(0), Vec::<Modifier>::new());
        assert_eq!(modifiers_from_mask(0b0001), vec![Modifier::Control]);
        assert_eq!(
            modifiers_from_mask(0b1111),
            vec![
                Modifier::Control,
                Modifier::Alt,
                Modifier::Shift,
                Modifier::Meta
            ]
        );
        // The order is the writing order, not the order the bits were set.
        assert_eq!(
            modifiers_from_mask(0b0110),
            vec![Modifier::Alt, Modifier::Shift]
        );
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
