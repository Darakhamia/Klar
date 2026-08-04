//! The permissions onboarding has to walk the user through.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    /// Both platforms. Without it there is no audio at all.
    Microphone,
    /// macOS only. Required for both the event tap and text injection, and
    /// bound to the code signature — an unsigned dev build loses it on every
    /// rebuild.
    Accessibility,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Microphone => "microphone",
            Self::Accessibility => "accessibility",
        };
        f.pad(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionState {
    Granted,
    Denied,
    /// Never asked for, or the OS will not say.
    Unknown,
    /// This platform does not gate the capability at all — Accessibility on
    /// Windows, for instance.
    NotApplicable,
}
