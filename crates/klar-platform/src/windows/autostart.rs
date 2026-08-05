//! "Start with Windows", which is a registry value and nothing more.
//!
//! `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run` is the
//! user's own hive: no elevation, no service, no scheduled task, and the user
//! can see and remove it from Task Manager's Startup tab. The alternatives —
//! `HKLM`, a service, a Startup-folder shortcut — all need more privilege or
//! leave something behind that Klar cannot clean up on uninstall.
//!
//! The value is rewritten rather than trusted: an update that installs to a new
//! path would otherwise leave a Run entry pointing at a binary that is gone.

use crate::PlatformError;
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::HSTRING;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// The value name. Matches the product name so it is recognisable in Task
/// Manager's Startup tab, which is where users go to turn this off.
const VALUE: &str = "Klar";

/// An open registry key, closed on drop.
struct Key(HKEY);

impl Key {
    fn open(
        access: windows::Win32::System::Registry::REG_SAM_FLAGS,
    ) -> Result<Self, PlatformError> {
        let mut key = HKEY::default();
        let path = HSTRING::from(RUN_KEY);

        // SAFETY: `path` outlives the call and `key` is a valid out-pointer.
        // The Run key always exists, so this never has to create it.
        let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, &path, Some(0), access, &mut key) };
        status
            .ok()
            .map_err(|error| PlatformError::Os(format!("could not open the Run key: {error}")))?;

        Ok(Self(key))
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: closes a key this guard opened, exactly once.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

pub fn launch_at_login() -> Result<bool, PlatformError> {
    let key = Key::open(KEY_READ)?;
    let name = HSTRING::from(VALUE);

    // SAFETY: querying for existence only — every out-pointer is null, which
    // is how this API is asked "is there a value called this".
    let status = unsafe { RegQueryValueExW(key.0, &name, None, None, None, None) };

    if status == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    status
        .ok()
        .map_err(|error| PlatformError::Os(format!("could not read the Run key: {error}")))?;

    Ok(true)
}

pub fn set_launch_at_login(on: bool) -> Result<(), PlatformError> {
    let key = Key::open(KEY_SET_VALUE)?;
    let name = HSTRING::from(VALUE);

    if !on {
        // SAFETY: deleting a value by name from a key open for writing.
        let status = unsafe { RegDeleteValueW(key.0, &name) };
        if status == ERROR_FILE_NOT_FOUND {
            // Already off. Turning off something that is already off is not an
            // error the user should ever see.
            return Ok(());
        }
        return status
            .ok()
            .map_err(|error| PlatformError::Os(format!("could not clear the Run key: {error}")));
    }

    let exe = std::env::current_exe()
        .map_err(|error| PlatformError::Os(format!("could not find our own path: {error}")))?;

    // Quoted, because Windows splits an unquoted Run value on spaces and
    // `C:\Program Files\Klar\klar.exe` would be run as `C:\Program`.
    //
    // `--hidden` is what makes this bearable: starting with Windows should put
    // Klar in the tray, not put a settings window in front of somebody who has
    // just logged in and is trying to open something else.
    let command = format!("\"{}\" --hidden", exe.display());

    // REG_SZ data includes the terminator, so it is pushed rather than assumed.
    let mut wide: Vec<u16> = command.encode_utf16().collect();
    wide.push(0);

    let bytes = {
        let ptr = wide.as_ptr().cast::<u8>();
        // SAFETY: `wide` is alive for this block and `len * 2` is exactly its
        // size in bytes.
        unsafe { std::slice::from_raw_parts(ptr, wide.len() * 2) }
    };

    // SAFETY: `name` and `bytes` outlive the call, and the length matches the
    // buffer exactly.
    let status = unsafe { RegSetValueExW(key.0, &name, Some(0), REG_SZ, Some(bytes)) };
    status
        .ok()
        .map_err(|error| PlatformError::Os(format!("could not write the Run key: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the real registry, under the current user's own hive and under a
    /// value name nothing else uses. It restores whatever it found.
    #[test]
    fn the_run_value_can_be_set_read_and_cleared() {
        let was = launch_at_login().expect("reading the Run key");

        set_launch_at_login(true).expect("setting the Run key");
        assert!(launch_at_login().expect("reading back"));

        set_launch_at_login(false).expect("clearing the Run key");
        assert!(!launch_at_login().expect("reading back"));

        set_launch_at_login(false).expect("clearing twice must not fail");

        if was {
            set_launch_at_login(true).expect("restoring what was there");
        }
    }
}
