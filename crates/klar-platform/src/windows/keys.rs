//! Mapping Klar's platform-independent [`Binding`] onto Windows virtual keys.

use crate::{Binding, Key, Modifier};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_F1, VK_F2, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7,
    VK_F8, VK_F9, VK_F10, VK_F11, VK_F12, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
    VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE,
};

/// The virtual key a binding's trigger corresponds to, or `None` for keys that
/// do not exist on Windows.
pub fn virtual_key(key: Key) -> Option<VIRTUAL_KEY> {
    Some(match key {
        Key::Space => VK_SPACE,
        Key::F1 => VK_F1,
        Key::F2 => VK_F2,
        Key::F3 => VK_F3,
        Key::F4 => VK_F4,
        Key::F5 => VK_F5,
        Key::F6 => VK_F6,
        Key::F7 => VK_F7,
        Key::F8 => VK_F8,
        Key::F9 => VK_F9,
        Key::F10 => VK_F10,
        Key::F11 => VK_F11,
        Key::F12 => VK_F12,
        // macOS only; there is no Windows equivalent to bind.
        Key::Fn => return None,
    })
}

/// The [`Key`] a virtual key code corresponds to, or `None` for the ones Klar
/// will not bind. The inverse of [`virtual_key`], used while capturing a chord.
pub fn key_from_virtual(code: u32) -> Option<Key> {
    const KEYS: &[Key] = &[
        Key::Space,
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
    ];
    KEYS.iter()
        .copied()
        .find(|key| virtual_key(*key).is_some_and(|vk| u32::from(vk.0) == code))
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
    fn every_bindable_key_has_a_virtual_key() {
        for key in [
            Key::Space,
            Key::F1,
            Key::F2,
            Key::F3,
            Key::F4,
            Key::F5,
            Key::F6,
            Key::F7,
            Key::F8,
            Key::F9,
            Key::F10,
            Key::F11,
            Key::F12,
        ] {
            assert!(virtual_key(key).is_some(), "{key:?} has no VK");
        }
    }

    #[test]
    fn virtual_keys_round_trip_back_to_their_key() {
        for key in [Key::Space, Key::F1, Key::F9, Key::F12] {
            let vk = virtual_key(key).expect("bindable");
            assert_eq!(key_from_virtual(u32::from(vk.0)), Some(key));
        }
    }

    #[test]
    fn keys_klar_will_not_bind_have_no_mapping() {
        // 'A'. Bindable in principle, deliberately not offered: a held letter
        // key is swallowed for as long as the dictation runs.
        assert_eq!(key_from_virtual(0x41), None);
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
            key: Key::F1,
        };
        assert!(modifiers_held(&binding));
    }
}
