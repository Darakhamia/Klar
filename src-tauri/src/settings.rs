//! What the user has chosen, on disk.
//!
//! A small JSON file in the app data directory. Deliberately not SQLite: this
//! is a dozen fields read once at startup, and the database in M5 is for
//! dictations, the dictionary and statistics — things there are thousands of.

use klar_core::asr::Accuracy;
use klar_core::polish::{OllamaConfig, Strength};
use klar_platform::Binding;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub cleanup: Strength,
    /// How hard whisper works on words that sound like other words.
    ///
    /// Fast by default, which is what M3 measured. Accurate costs time on every
    /// dictation and buys back exactly the case a dictionary exists for: a name
    /// the model has never seen.
    pub accuracy: Accuracy,
    /// Where the local model server is, and which of its models to use.
    ///
    /// An empty model name is the honest default: whatever a machine has pulled
    /// is what works, and guessing a name would send a first-time user after a
    /// download that may be the wrong one. Until it is set, polish stays off
    /// whatever the cleanup strength says.
    pub polish_endpoint: String,
    pub polish_model: String,
    pub appearance: Appearance,
    /// Whether Klar asks the update server, at startup, whether there is a
    /// newer version.
    ///
    /// On by default. The request carries Klar's version and nothing about
    /// what was dictated — see `updates.rs` — and an unsigned app with no way
    /// to ship a fix is worse for the people using it than one that asks a
    /// server for a version number. Switchable, and the row says exactly that.
    pub check_for_updates: bool,
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
            cleanup: Strength::default(),
            accuracy: Accuracy::default(),
            polish_endpoint: klar_core::polish::ollama::DEFAULT_ENDPOINT.to_owned(),
            polish_model: String::new(),
            appearance: Appearance::default(),
            check_for_updates: true,
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

    /// What the polish stage should be, given what the user chose and what is
    /// actually configured.
    ///
    /// `None` means the transcript goes straight through. That is what verbatim
    /// asks for, and also what an unset model has to mean: an endpoint with
    /// nothing behind it would fail on every dictation, and failing quietly to
    /// plain text beats failing loudly at the moment somebody speaks.
    pub fn polish(&self) -> Option<OllamaConfig> {
        if self.cleanup == Strength::Verbatim || self.polish_model.is_empty() {
            return None;
        }
        Some(OllamaConfig {
            endpoint: self.polish_endpoint.clone(),
            model: self.polish_model.clone(),
            ..OllamaConfig::default()
        })
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
