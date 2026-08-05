//! Hold-to-talk on Windows, via a low-level keyboard hook.
//!
//! `RegisterHotKey` is not usable here: it reports the press and never the
//! release, and the release is the half that ends a dictation. `WH_KEYBOARD_LL`
//! gives both, at the cost of some care:
//!
//! - The hook must live on a thread with its own message pump. Windows delivers
//!   hook callbacks by pumping messages on the installing thread.
//! - **The callback blocks the entire system's input queue while it runs.** If
//!   it takes longer than `LowLevelHooksTimeout` (300 ms by default) Windows
//!   silently removes the hook. So the callback does the minimum — compare a
//!   key code, read modifier state, push onto a channel — and the user's
//!   callback runs on a separate dispatch thread.
//! - It must never panic. A panic here would unwind into a Windows callback,
//!   which is undefined behaviour, and would take down a background app the
//!   user cannot see.

use super::keys;
use crate::{BadBinding, Binding, Hotkey, HotkeyEvent, PlatformError};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::time::Duration;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, MSG, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

/// What the hook callback needs to do its job. Lives in a thread-local on the
/// hook thread — the callback runs there and nowhere else, so no lock is taken
/// on the input path.
struct HookContext {
    binding: Binding,
    trigger: u32,
    /// True between a press we reported and its release. Key repeat sends a
    /// stream of `WM_KEYDOWN` while a key is held; only the first is a press.
    holding: bool,
    events: Sender<HotkeyEvent>,
}

thread_local! {
    static CONTEXT: RefCell<Option<HookContext>> = const { RefCell::new(None) };
}

pub struct WindowsHotkey {
    /// Thread id of the hook thread, so `unregister` can post it a `WM_QUIT`.
    /// Zero when nothing is registered.
    hook_thread: AtomicU32,
    threads: Vec<std::thread::JoinHandle<()>>,
}

impl WindowsHotkey {
    pub const fn new() -> Self {
        Self {
            hook_thread: AtomicU32::new(0),
            threads: Vec::new(),
        }
    }
}

impl Hotkey for WindowsHotkey {
    fn register(
        &mut self,
        binding: &Binding,
        mut on_event: Box<dyn FnMut(HotkeyEvent) + Send>,
    ) -> Result<(), PlatformError> {
        if self.hook_thread.load(Ordering::SeqCst) != 0 {
            self.unregister()?;
        }

        let trigger = keys::virtual_key(binding.key).ok_or_else(|| {
            PlatformError::Hotkey(format!("{:?} cannot be bound on Windows", binding.key))
        })?;

        let (events_tx, events_rx) = channel::<HotkeyEvent>();
        let (ready_tx, ready_rx) = channel::<Result<u32, String>>();
        let described = format!("{:?}+{:?}", binding.modifiers, binding.key);
        let binding = binding.clone();

        let hook_thread = std::thread::Builder::new()
            .name("klar-hotkey".into())
            .spawn(move || run_hook(binding, u32::from(trigger.0), events_tx, ready_tx))
            .map_err(|e| PlatformError::Hotkey(e.to_string()))?;

        // The dispatch thread exists so the hook callback never runs user code.
        // Anything slow there would stall every keystroke on the machine.
        let dispatch_thread = std::thread::Builder::new()
            .name("klar-hotkey-dispatch".into())
            .spawn(move || {
                for event in events_rx {
                    on_event(event);
                }
            })
            .map_err(|e| PlatformError::Hotkey(e.to_string()))?;

        match ready_rx.recv() {
            Ok(Ok(thread_id)) => {
                self.hook_thread.store(thread_id, Ordering::SeqCst);
                self.threads.push(hook_thread);
                self.threads.push(dispatch_thread);
                tracing::info!(binding = %described, "push-to-talk hook installed");
                Ok(())
            }
            Ok(Err(reason)) => Err(PlatformError::Hotkey(reason)),
            Err(_) => Err(PlatformError::Hotkey(
                "the hook thread died on startup".into(),
            )),
        }
    }

    fn unregister(&mut self) -> Result<(), PlatformError> {
        let thread_id = self.hook_thread.swap(0, Ordering::SeqCst);
        if thread_id != 0 {
            // Ends the GetMessageW loop, which unhooks and drops the sender,
            // which in turn ends the dispatch thread.
            // SAFETY: posting a quit message to a thread id we own.
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        for thread in self.threads.drain(..) {
            if thread.join().is_err() {
                tracing::error!("a hotkey thread panicked");
            }
        }
        Ok(())
    }
}

impl Drop for WindowsHotkey {
    fn drop(&mut self) {
        let _ = self.unregister();
    }
}

/// What the capture hook sends back: the key that completed the chord, and the
/// modifiers that were down at that instant.
///
/// A bitmask rather than a `Vec<Modifier>` because this is built inside the
/// hook callback, which runs in the system's input path and does not allocate.
type Chord = (u32, u8);

thread_local! {
    static CAPTURE: RefCell<Option<Sender<Chord>>> = const { RefCell::new(None) };
}

/// Wait for one chord and report it. See [`crate::capture`].
///
/// A second, temporary hook rather than a mode on the push-to-talk one: the two
/// have opposite jobs — that one swallows a known key forever, this one
/// swallows one unknown key once — and the caller has stopped the engine, so
/// there is nothing for them to fight over.
pub fn capture_binding(timeout: Duration) -> Result<Binding, PlatformError> {
    let (chord_tx, chord_rx) = channel::<Chord>();
    let (ready_tx, ready_rx) = channel::<Result<u32, String>>();

    let thread = std::thread::Builder::new()
        .name("klar-hotkey-capture".into())
        .spawn(move || run_capture_hook(chord_tx, ready_tx))
        .map_err(|e| PlatformError::Hotkey(e.to_string()))?;

    let thread_id = match ready_rx.recv() {
        Ok(Ok(id)) => id,
        Ok(Err(reason)) => return Err(PlatformError::Hotkey(reason)),
        Err(_) => {
            return Err(PlatformError::Hotkey(
                "the capture thread died on startup".into(),
            ));
        }
    };

    let outcome = chord_rx.recv_timeout(timeout);

    // Whatever happened, the hook comes down before this returns. Leaving a
    // low-level keyboard hook installed would swallow the next key pressed
    // anywhere on the machine.
    // SAFETY: posting a quit message to a thread id we own.
    unsafe {
        let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
    }
    if thread.join().is_err() {
        tracing::error!("the hotkey capture thread panicked");
    }

    let (code, mask) = match outcome {
        Ok(chord) => chord,
        Err(RecvTimeoutError::Timeout) => return Err(BadBinding::TimedOut.into()),
        Err(RecvTimeoutError::Disconnected) => {
            return Err(PlatformError::Hotkey("the capture hook stopped".into()));
        }
    };

    if code == u32::from(VK_ESCAPE.0) {
        return Err(BadBinding::Cancelled.into());
    }

    let key = keys::key_from_virtual(code).ok_or(BadBinding::UnsupportedKey)?;
    let binding = Binding {
        modifiers: keys::modifiers_from_mask(mask),
        key,
    };
    binding.check()?;

    tracing::info!(?binding, "captured a new push-to-talk binding");
    Ok(binding)
}

/// Body of the capture thread: install, pump until the chord or a quit, unhook.
fn run_capture_hook(chord: Sender<Chord>, ready: Sender<Result<u32, String>>) {
    CAPTURE.with(|slot| {
        *slot.borrow_mut() = Some(chord);
    });

    // SAFETY: installs a global keyboard hook with a valid callback. A null
    // module handle is correct for a hook whose procedure lives in this process.
    let hook = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(capture_proc), None, 0) } {
        Ok(hook) => hook,
        Err(error) => {
            let _ = ready.send(Err(format!("SetWindowsHookExW failed: {error}")));
            return;
        }
    };

    // SAFETY: no arguments, returns the calling thread's id.
    let thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    if ready.send(Ok(thread_id)).is_ok() {
        pump_until_quit();
    }

    // SAFETY: same handle, released exactly once.
    let _ = unsafe { UnhookWindowsHookEx(hook) };
    CAPTURE.with(|slot| {
        *slot.borrow_mut() = None;
    });
}

/// The capture callback. Same rules as [`hook_proc`]: bounded work, no locks,
/// no user code, no panics.
unsafe extern "system" fn capture_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        // SAFETY: forwarding the parameters we were given, unmodified.
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    let swallow = CAPTURE.with(|slot| {
        // `try_borrow_mut` rather than `borrow_mut`: a panic here would unwind
        // into a Windows callback, which is undefined behaviour.
        let Ok(slot) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(chord) = slot.as_ref() else {
            return false;
        };

        if !matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
            return false;
        }

        // SAFETY: for WH_KEYBOARD_LL with code >= 0, lParam is a pointer to a
        // KBDLLHOOKSTRUCT owned by the system for the duration of this call.
        let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };

        // A modifier on its own completes nothing — the user is still building
        // the chord, and it must reach the OS so the next key sees it held.
        if keys::is_modifier_key(event.vkCode) {
            return false;
        }

        // Swallow whatever it was. The user is pressing this at a settings
        // window, and an F-key or a Space arriving there would do something.
        let _ = chord.send((event.vkCode, keys::held_mask()));
        true
    });

    if swallow {
        return LRESULT(1);
    }

    // SAFETY: forwarding the parameters we were given, unmodified.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Body of the hook thread: install, pump, unhook.
fn run_hook(
    binding: Binding,
    trigger: u32,
    events: Sender<HotkeyEvent>,
    ready: Sender<Result<u32, String>>,
) {
    CONTEXT.with(|slot| {
        *slot.borrow_mut() = Some(HookContext {
            binding,
            trigger,
            holding: false,
            events,
        });
    });

    // SAFETY: installs a global keyboard hook with a valid callback. A null
    // module handle is correct for a hook whose procedure lives in this process.
    let hook = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) } {
        Ok(hook) => hook,
        Err(error) => {
            let _ = ready.send(Err(format!("SetWindowsHookExW failed: {error}")));
            return;
        }
    };

    // SAFETY: no arguments, returns the calling thread's id.
    let thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    if ready.send(Ok(thread_id)).is_err() {
        // SAFETY: unhooking a handle we installed and have not yet released.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        return;
    }

    pump_until_quit();

    // SAFETY: same handle, released exactly once.
    let _ = unsafe { UnhookWindowsHookEx(hook) };
    CONTEXT.with(|slot| {
        *slot.borrow_mut() = None;
    });
}

/// Run the message loop until a `WM_QUIT` arrives.
fn pump_until_quit() {
    let mut message = MSG::default();
    loop {
        // SAFETY: `message` is a valid, writable MSG for the duration of the
        // call. A zero return means WM_QUIT; -1 means an error we treat the
        // same, since there is nothing useful to retry.
        let result = unsafe { GetMessageW(&raw mut message, None, 0, 0) };
        if result.0 <= 0 {
            return;
        }
    }
}

/// The hook callback. Runs on the hook thread, inside the system input path.
///
/// Everything in here is bounded work: no allocation beyond the channel send,
/// no locks, no user code, no panics.
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Negative codes must be passed straight through without inspection.
    if code < 0 {
        // SAFETY: forwarding the parameters we were given, unmodified.
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    let swallow = CONTEXT.with(|slot| {
        // `try_borrow_mut` rather than `borrow_mut`: a panic on this path would
        // unwind into a Windows callback, which is undefined behaviour.
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(context) = slot.as_mut() else {
            return false;
        };

        // SAFETY: for WH_KEYBOARD_LL with code >= 0, lParam is a pointer to a
        // KBDLLHOOKSTRUCT owned by the system for the duration of this call.
        let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if event.vkCode != context.trigger {
            return false;
        }

        match wparam.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if context.holding {
                    // Key repeat. Already recording; swallow it and say nothing.
                    return true;
                }
                if !keys::modifiers_held(&context.binding) {
                    // The trigger without its modifiers is just that key. Let it
                    // through — swallowing every Space would be a disaster.
                    return false;
                }
                context.holding = true;
                let _ = context.events.send(HotkeyEvent::Pressed);
                true
            }
            WM_KEYUP | WM_SYSKEYUP => {
                if !context.holding {
                    return false;
                }
                // Deliberately not checking modifiers here: releasing Ctrl
                // before Space is normal, and the dictation must still end.
                context.holding = false;
                let _ = context.events.send(HotkeyEvent::Released);
                true
            }
            _ => false,
        }
    });

    if swallow {
        // Non-zero stops the key reaching the focused application, so dictating
        // into an editor does not also type into it.
        return LRESULT(1);
    }

    // SAFETY: forwarding the parameters we were given, unmodified.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
