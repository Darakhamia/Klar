//! Getting text into whatever application is focused.
//!
//! The primary path is the clipboard: save what is there, set ours, synthesise
//! the paste chord, restore. It is the only approach that handles Unicode,
//! emoji and long text reliably. Synthesised keystrokes stay as a fallback for
//! applications that block programmatic paste.
//!
//! The user's clipboard must survive every path through this module, including
//! the error path.

use crate::PlatformError;

/// How the text was delivered. Logged, and surfaced in the UI when the fallback
/// had to be used so a failing application can be identified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionMethod {
    Clipboard,
    Keystrokes,
}

pub trait TextInjector: Send {
    /// Insert `text` at the cursor in the focused application, choosing the
    /// method.
    ///
    /// Implementations must restore the previous clipboard contents whether they
    /// succeed or fail.
    fn inject(&mut self, text: &str) -> Result<InjectionMethod, PlatformError>;

    /// Insert `text` using a specific method.
    ///
    /// Some applications refuse a programmatic paste, and the only way to know
    /// is to try. This is what the per-application override in settings sets,
    /// and what `klar-cli inject --method keystrokes` exercises.
    fn inject_using(
        &mut self,
        text: &str,
        method: InjectionMethod,
    ) -> Result<InjectionMethod, PlatformError>;
}
