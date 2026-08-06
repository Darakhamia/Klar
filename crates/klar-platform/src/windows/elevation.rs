//! Detecting the case where injection would silently do nothing.
//!
//! Windows refuses input from a lower-integrity process to a higher-integrity
//! window — User Interface Privilege Isolation. `SendInput` returns success and
//! the keystrokes go nowhere. Without this check the app looks like it hung.

use crate::PlatformError;
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE};
use windows::Win32::Security::{
    GetTokenInformation, TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenIntegrityLevel,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// What we can tell about the window that will receive the paste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Injection should work.
    Reachable,
    /// Higher integrity than us: input will be dropped without an error.
    Elevated,
    /// Nothing is focused, or the OS would not say. Try anyway — refusing on a
    /// maybe would be worse than a paste that does not land.
    Unknown,
}

/// Inspect the foreground window's process.
pub fn foreground_target() -> Target {
    // SAFETY: no arguments; returns null when nothing is focused.
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return Target::Unknown;
    }

    let mut pid = 0_u32;
    // SAFETY: `window` is a valid HWND and `pid` is a valid out-pointer.
    unsafe { GetWindowThreadProcessId(window, Some(&raw mut pid)) };
    if pid == 0 {
        return Target::Unknown;
    }

    // SAFETY: opening another process with the most limited query right there
    // is. Access denied here is itself the answer we are looking for.
    let process = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(handle) => handle,
        Err(error) => {
            return if error.code() == ERROR_ACCESS_DENIED.to_hresult() {
                // A non-elevated process cannot open an elevated one even for a
                // limited query. Protected processes look the same, and the
                // outcome for injection is identical.
                Target::Elevated
            } else {
                Target::Unknown
            };
        }
    };

    let target_level = integrity_level(process);
    // SAFETY: closing a handle we opened, exactly once.
    let _ = unsafe { CloseHandle(process) };

    // SAFETY: a pseudo-handle to the current process; it needs no closing.
    let ours = integrity_level(unsafe { GetCurrentProcess() });

    match (ours, target_level) {
        (Some(ours), Some(theirs)) if theirs > ours => Target::Elevated,
        (Some(_), Some(_)) => Target::Reachable,
        _ => Target::Unknown,
    }
}

/// The name of the focused application, for labelling a dictation in the
/// history. `notepad.exe` comes back as `Notepad`.
///
/// Every failure is `None`. A protected process will not answer, a process
/// running as another user will not answer, and neither is worth an error on a
/// dictation that already landed correctly.
pub fn foreground_app() -> Option<String> {
    // SAFETY: no arguments; returns null when nothing is focused.
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return None;
    }

    let mut pid = 0_u32;
    // SAFETY: `window` is a valid HWND and `pid` is a valid out-pointer.
    unsafe { GetWindowThreadProcessId(window, Some(&raw mut pid)) };
    if pid == 0 {
        return None;
    }

    // SAFETY: the most limited query right there is; an elevated or protected
    // target simply refuses, which is one of the `None` cases.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

    let mut buffer = [0_u16; 260];
    let mut length = buffer.len() as u32;
    // SAFETY: `buffer` is `length` wide characters and both pointers are valid
    // for the call. On success `length` is set to the characters written.
    let queried = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &raw mut length,
        )
    };

    // SAFETY: closing a handle we opened, exactly once.
    let _ = unsafe { CloseHandle(process) };
    queried.ok()?;

    let path = String::from_utf16_lossy(&buffer[..length as usize]);
    let stem = std::path::Path::new(&path).file_stem()?.to_str()?;
    if stem.is_empty() {
        return None;
    }

    // `notepad` reads better than `notepad.exe` in a list, and capitalising the
    // first letter covers the majority of Windows executables, whose names are
    // lowercase where the application's is not.
    let mut chars = stem.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().collect::<String>() + chars.as_str())
}

/// The mandatory integrity level of a process, as the RID of its integrity SID.
/// Higher is more privileged: 0x2000 medium, 0x3000 high, 0x4000 system.
fn integrity_level(process: HANDLE) -> Option<u32> {
    let mut token = HANDLE::default();
    // SAFETY: `process` is a valid handle and `token` is a valid out-pointer.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut token) }.ok()?;

    let level = read_integrity_level(token);

    // SAFETY: closing the token we just opened, exactly once.
    let _ = unsafe { CloseHandle(token) };
    level
}

fn read_integrity_level(token: HANDLE) -> Option<u32> {
    let mut needed = 0_u32;
    // First call asks for the size. It is expected to fail; only `needed`
    // matters.
    // SAFETY: passing None for the buffer is the documented way to size it.
    let _ = unsafe { GetTokenInformation(token, TokenIntegrityLevel, None, 0, &raw mut needed) };
    if needed == 0 {
        return None;
    }

    let mut buffer = vec![0_u8; needed as usize];
    // SAFETY: `buffer` is exactly `needed` bytes, which is what the call above
    // asked for.
    unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            &raw mut needed,
        )
    }
    .ok()?;

    // SAFETY: on success the buffer holds a TOKEN_MANDATORY_LABEL whose Label
    // .Sid points into the same allocation.
    let label = unsafe { &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>() };
    let sid = label.Label.Sid;
    if sid.is_invalid() {
        return None;
    }

    // The integrity level is the last sub-authority of the SID.
    // SAFETY: `sid` is a valid SID for the lifetime of `buffer`.
    let count = unsafe { *windows::Win32::Security::GetSidSubAuthorityCount(sid) };
    if count == 0 {
        return None;
    }
    // SAFETY: index is in range, having just read the count.
    let level = unsafe { *windows::Win32::Security::GetSidSubAuthority(sid, u32::from(count) - 1) };
    Some(level)
}

/// Turn the verdict into the error the UI shows, or `Ok` to go ahead.
pub fn check_foreground() -> Result<(), PlatformError> {
    match foreground_target() {
        Target::Elevated => Err(PlatformError::ElevatedTarget),
        Target::Reachable => Ok(()),
        Target::Unknown => {
            tracing::debug!(
                "could not determine the foreground window's integrity; injecting anyway"
            );
            Ok(())
        }
    }
}
