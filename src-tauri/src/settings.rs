//! What the user has chosen, on disk.
//!
//! A small JSON file in the app data directory. Deliberately not SQLite: this
//! is a dozen fields read once at startup, and the database in M5 is for
//! dictations, the dictionary and statistics — things there are thousands of.

use klar_platform::Binding;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

/// Broadcast when the settings change, so every window follows without having
/// to poll or be told individually. The theme rides on this.
pub const EVENT: &str = "klar://settings";

pub fn broadcast(app: &AppHandle, settings: &Settings) {
    if let Err(error) = app.emit(EVENT, settings) {
        tracing::warn!(%error, "could not tell the windows about a settings change");
    }
}

/// Where the finished text goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum FinishAction {
    #[default]
    Type,
    Copy,
    TypeAndCopy,
}

/// Which of the two shells the windows are drawn in.
///
/// The design ships both. `System` follows the OS, which is what a resident
/// app should do unless told otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum OverlayPosition {
    #[default]
    BottomCentre,
    NearCursor,
    TopCentre,
}

/// Where speech is processed. Local is the default and cloud is opt-in — the
/// privacy rule in CLAUDE.md, expressed as a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Processing {
    #[default]
    Local,
    Cloud,
}

/// How much the polish stage is allowed to rewrite. Wired up in M4; stored now
/// so the setting survives the milestone that gives it meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Cleanup {
    Verbatim,
    Light,
    #[default]
    Balanced,
    Heavy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub hotkey: Binding,
    pub launch_at_login: bool,
    pub on_finish: FinishAction,
    pub overlay_position: OverlayPosition,
    /// ISO code, or `None` for automatic detection. Pinning it saves whisper's
    /// detection pass — 114 ms measured — so the interface should encourage it.
    pub language: Option<String>,
    pub processing: Processing,
    pub model: String,
    /// Device name, or `None` for the system default.
    pub microphone: Option<String>,
    pub cleanup: Cleanup,
    pub appearance: Appearance,
    /// Whether first-run setup has been completed. False on a fresh install and
    /// on an install that predates onboarding — running through it again costs
    /// a few seconds when everything is already downloaded.
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: klar_platform::default_binding(),
            launch_at_login: false,
            on_finish: FinishAction::default(),
            overlay_position: OverlayPosition::default(),
            language: None,
            processing: Processing::default(),
            model: klar_core::model::DEFAULT_MODEL.to_owned(),
            microphone: None,
            cleanup: Cleanup::default(),
            appearance: Appearance::default(),
            onboarded: false,
        }
    }
}

impl Settings {
    /// Read from disk, falling back to the defaults.
    ///
    /// A settings file that will not parse is replaced rather than fatal: the
    /// app has to start, and every field has a sensible default.
    pub fn load() -> Self {
        let Some(path) = path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(settings) => settings,
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "settings file unreadable; using defaults");
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                tracing::warn!(%error, "could not read settings; using defaults");
                Self::default()
            }
        }
    }

    /// Write to disk, through a temporary file so an interrupted save cannot
    /// leave a half-written settings file behind.
    pub fn save(&self) -> Result<(), String> {
        let path = path().ok_or("no app data directory")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, text).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, &path).map_err(|e| e.to_string())
    }
}

fn path() -> Option<PathBuf> {
    directories::ProjectDirs::from("app", "Klar", "Klar")
        .map(|dirs| dirs.config_local_dir().join("settings.json"))
}
