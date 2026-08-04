//! Saving, replacing and restoring the Windows clipboard.
//!
//! Klar borrows the clipboard to paste. The user's contents must come back
//! whatever happens — success, failure, or a panic elsewhere in the pipeline —
//! so everything here is written around [`Snapshot`], which restores what it
//! captured and reports honestly about what it could not.

use crate::PlatformError;
use std::time::Duration;
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};

const CF_UNICODETEXT: u32 = 13;

/// Another process can hold the clipboard open. It is normally released within
/// a frame, so a short retry beats failing the dictation.
const OPEN_ATTEMPTS: u32 = 10;
const OPEN_RETRY: Duration = Duration::from_millis(10);

/// One clipboard format and its bytes.
struct Entry {
    format: u32,
    bytes: Vec<u8>,
}

/// What was on the clipboard before Klar touched it.
pub struct Snapshot {
    entries: Vec<Entry>,
    /// Formats that were present but could not be copied — GDI handles such as
    /// `CF_BITMAP`, and anything rendered on demand by its owner. Restoring
    /// cannot bring these back, so the caller is told rather than left to
    /// discover it.
    skipped: Vec<u32>,
}

impl Snapshot {
    pub fn is_complete(&self) -> bool {
        self.skipped.is_empty()
    }

    pub fn skipped_formats(&self) -> &[u32] {
        &self.skipped
    }
}

/// A clipboard opened for the lifetime of the guard, and closed on drop even if
/// the body between returns early.
struct ClipboardGuard;

impl ClipboardGuard {
    fn open() -> Result<Self, PlatformError> {
        let mut last = String::new();
        for _ in 0..OPEN_ATTEMPTS {
            // SAFETY: a null owner window is valid and ties the clipboard to
            // the current task rather than a window we do not have.
            match unsafe { OpenClipboard(None) } {
                Ok(()) => return Ok(Self),
                Err(error) => {
                    last = error.to_string();
                    std::thread::sleep(OPEN_RETRY);
                }
            }
        }
        Err(PlatformError::Clipboard(format!(
            "could not open the clipboard after {OPEN_ATTEMPTS} attempts: {last}"
        )))
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: closes a clipboard this guard opened, exactly once.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Copy everything currently on the clipboard that can be copied.
pub fn snapshot() -> Result<Snapshot, PlatformError> {
    let _guard = ClipboardGuard::open()?;

    let mut entries = Vec::new();
    let mut skipped = Vec::new();
    let mut format = 0_u32;

    loop {
        // SAFETY: iterates the clipboard's format list; zero starts and ends it.
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            break;
        }

        // SAFETY: the returned handle belongs to the clipboard. We read it and
        // must not free it.
        let Ok(handle) = (unsafe { GetClipboardData(format) }) else {
            skipped.push(format);
            continue;
        };

        match read_global(handle) {
            Some(bytes) => entries.push(Entry { format, bytes }),
            None => skipped.push(format),
        }
    }

    if !skipped.is_empty() {
        tracing::warn!(
            formats = ?skipped,
            "clipboard formats that cannot be restored were on the clipboard"
        );
    }

    Ok(Snapshot { entries, skipped })
}

/// Put `text` on the clipboard, replacing what is there.
pub fn set_text(text: &str) -> Result<(), PlatformError> {
    let mut utf16: Vec<u16> = text.encode_utf16().collect();
    utf16.push(0);

    let bytes = {
        // Reinterpreting the UTF-16 buffer as the bytes CF_UNICODETEXT wants.
        let ptr = utf16.as_ptr().cast::<u8>();
        // SAFETY: `utf16` is alive for this block and `len * 2` is exactly its
        // size in bytes.
        unsafe { std::slice::from_raw_parts(ptr, utf16.len() * 2) }
    };

    let _guard = ClipboardGuard::open()?;

    // SAFETY: we hold the clipboard open, which is the precondition.
    unsafe { EmptyClipboard() }.map_err(|e| PlatformError::Clipboard(e.to_string()))?;

    write_global(CF_UNICODETEXT, bytes)
}

impl Snapshot {
    /// Put the captured contents back.
    ///
    /// Called on every path out of injection, including the failing ones.
    pub fn restore(&self) -> Result<(), PlatformError> {
        let _guard = ClipboardGuard::open()?;

        // SAFETY: we hold the clipboard open.
        unsafe { EmptyClipboard() }.map_err(|e| PlatformError::Clipboard(e.to_string()))?;

        let mut failures = Vec::new();
        for entry in &self.entries {
            if let Err(error) = write_global(entry.format, &entry.bytes) {
                failures.push(format!("{}: {error}", entry.format));
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(PlatformError::Clipboard(format!(
                "could not restore {} clipboard format(s): {}",
                failures.len(),
                failures.join(", ")
            )))
        }
    }
}

/// Copy a global memory handle's bytes. `None` for handles that are not global
/// memory at all — `CF_BITMAP` and friends.
fn read_global(handle: HANDLE) -> Option<Vec<u8>> {
    let global = HGLOBAL(handle.0);

    // SAFETY: GlobalSize returns 0 for anything that is not a global memory
    // handle, which is exactly the case we want to detect.
    let size = unsafe { GlobalSize(global) };
    if size == 0 {
        return None;
    }

    // SAFETY: locking a handle we have just confirmed is global memory.
    let ptr = unsafe { GlobalLock(global) };
    if ptr.is_null() {
        return None;
    }

    // SAFETY: `ptr` is valid for `size` bytes until the matching unlock below.
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), size) }.to_vec();

    // SAFETY: balances the lock above. The error it returns on the last unlock
    // is not a failure — `GlobalUnlock` reports the remaining lock count.
    let _ = unsafe { GlobalUnlock(global) };

    Some(bytes)
}

/// Allocate a global block, fill it, and hand ownership to the clipboard.
///
/// The clipboard must already be open and emptied by the caller.
fn write_global(format: u32, bytes: &[u8]) -> Result<(), PlatformError> {
    // SAFETY: allocating movable memory of a known size.
    let global = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }
        .map_err(|e| PlatformError::Clipboard(format!("GlobalAlloc: {e}")))?;

    // SAFETY: locking memory we just allocated.
    let ptr = unsafe { GlobalLock(global) };
    if ptr.is_null() {
        return Err(PlatformError::Clipboard("GlobalLock returned null".into()));
    }

    // SAFETY: `ptr` is valid for the `bytes.len()` we asked for, and the source
    // and destination cannot overlap — one was allocated a moment ago.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.cast::<u8>(), bytes.len()) };

    // SAFETY: balances the lock above.
    let _ = unsafe { GlobalUnlock(global) };

    // SAFETY: the clipboard is open. On success the system takes ownership of
    // the handle, so it must not be freed here.
    match unsafe { SetClipboardData(format, Some(HANDLE(global.0))) } {
        Ok(_) => Ok(()),
        Err(error) => {
            // Ownership did not transfer, so the block is ours to leak or free.
            // Leaking one clipboard-sized allocation on a failure path is
            // preferable to risking a double free against the system.
            Err(PlatformError::Clipboard(format!(
                "SetClipboardData({format}): {error}"
            )))
        }
    }
}
