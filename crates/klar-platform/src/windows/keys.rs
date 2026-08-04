//! Mapping Klar's platform-independent [`Binding`] onto Windows virtual keys.

use crate::{Binding, Key, Modifier};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_F1, VK_F2, VK_F3, VK_F4, VK_LWIN, VK_MENU,
    VK_RWIN, VK_SHIFT, VK_SPACE,
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
        // macOS only; there is no Windows equivalent to bind.
        Key::Fn => return None,
    })
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
    fn every_bindable_key_has_a_virtual_key() {
        for key in [Key::Space, Key::F1, Key::F2, Key::F3, Key::F4] {
            assert!(virtual_key(key).is_some(), "{key:?} has no VK");
        }
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
