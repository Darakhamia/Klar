//! Checking for a newer Klar, and installing it.
//!
//! Klar is unsigned and, until this existed, had no way to reach anybody who
//! had already installed it. A fix could be written and never arrive. That is
//! the problem this solves, and it is worth being precise about what it costs.
//!
//! **What leaves the machine.** A request for one file on the update server:
//! Klar's version, target and architecture appear in the URL because the server
//! needs them to answer. Nothing about what was dictated, how often, or by
//! whom. It is not telemetry — the server learns that a Klar of some version
//! asked, the same as any download would tell it.
//!
//! That is still a network request the user did not make, so it is switchable
//! and the switch says exactly this. It defaults to on: an unsigned app with no
//! way to ship a fix is worse for its users than one that asks a server for a
//! version number.
//!
//! **What is verified.** Every update is signed with a private key that never
//! leaves the machine that builds releases, and the public half is compiled
//! into the app. An installer that does not verify against it is refused
//! before it runs. This is separate from Windows code signing — see the README
//! — and it is what stops the update channel itself from being a way in.

use serde::Serialize;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

/// A newer version, when there is one.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    pub version: String,
    pub current: String,
    /// The release notes from the update manifest, when it carries any.
    pub notes: Option<String>,
    pub date: Option<String>,
}

/// Ask the update server whether there is something newer.
///
/// `Ok(None)` means this is the current version. An error here is ordinary —
/// no network, a server that is down, a build with no update channel
/// configured — and the interface reports it as a line of text rather than
/// anything alarming.
pub async fn check(app: &AppHandle) -> Result<Option<Available>, String> {
    let updater = app.updater().map_err(describe)?;

    match updater.check().await.map_err(describe)? {
        Some(update) => Ok(Some(Available {
            version: update.version.clone(),
            current: update.current_version.clone(),
            notes: update.body.clone(),
            date: update.date.map(|date| date.to_string()),
        })),
        None => Ok(None),
    }
}

/// Download, verify, install, and restart into the new version.
///
/// Checks again rather than holding the result of an earlier check: the two
/// are separate user actions with a person's decision in between, and a stale
/// handle from before that is not worth the state it would need.
///
/// This does not return on success — the app is replaced and restarted.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(describe)?;

    let Some(update) = updater.check().await.map_err(describe)? else {
        return Err("Klar is already up to date.".to_owned());
    };

    let version = update.version.clone();
    tracing::info!(%version, "downloading update");

    update
        .download_and_install(
            |_chunk, _total| {},
            || tracing::info!("update downloaded; installing"),
        )
        .await
        .map_err(describe)?;

    tracing::info!(%version, "restarting into the new version");
    app.restart();
}

/// Ask once, in the background, at startup.
///
/// Its own task so it cannot delay the tray, the window or the engine. A
/// failure is logged and nothing else: somebody starting a dictation app has
/// not asked to hear about the network.
pub fn check_in_background(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        match check(&handle).await {
            Ok(Some(available)) => {
                tracing::info!(
                    version = %available.version,
                    current = %available.current,
                    "an update is available"
                );
                announce(&handle, &available);
            }
            Ok(None) => tracing::info!("klar is up to date"),
            Err(error) => tracing::info!(%error, "could not check for updates"),
        }
    });
}

/// Tell the user, in the two places that reach somebody who is not looking.
///
/// The tray menu carries it for as long as it is true. The notification is the
/// part that gets noticed, and it is fired **once per version**: the check runs
/// at every startup, so without that it would be a toast every launch until the
/// user gave in, which is how an app teaches people to dismiss it unread. The
/// version last announced is remembered on disk beside the settings.
fn announce(app: &AppHandle, available: &Available) {
    crate::tray::announce_update(app, &available.version);

    if announced() == Some(available.version.clone()) {
        tracing::debug!(version = %available.version, "already announced; menu only");
        return;
    }

    let body = match available.notes.as_deref() {
        // The manifest's notes, trimmed to what a toast will show before the
        // system cuts it off mid-word.
        Some(notes) if !notes.trim().is_empty() => shorten(notes, 180),
        None | Some(_) => "Open Klar's settings to install it.".to_owned(),
    };

    match app
        .notification()
        .builder()
        .title(format!("Klar {} is available", available.version))
        .body(body)
        .show()
    {
        // Only after it was actually shown. A toast that failed — no Start menu
        // shortcut to hang the app identity on, a user who has notifications
        // switched off at the OS level — should be retried next launch rather
        // than counted as delivered.
        Ok(()) => remember(&available.version),
        Err(error) => tracing::warn!(%error, "could not show the update notification"),
    }
}

/// Cut on a word boundary, so the last thing the user reads is a word.
fn shorten(text: &str, limit: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= limit {
        return text.to_owned();
    }

    let cut: String = text.chars().take(limit).collect();
    let end = cut.rfind(' ').unwrap_or(cut.len());
    format!("{}…", cut[..end].trim_end_matches(['.', ',', ' ']))
}

/// The file holding the last version we sent a notification about.
///
/// Deliberately not a field in `Settings`: that struct is mirrored in
/// TypeScript and round-tripped through the settings window, so a field the
/// frontend does not know about is a field the next save silently drops.
fn announced_path() -> Option<PathBuf> {
    crate::settings::config_dir().map(|dir| dir.join("announced-version"))
}

fn announced() -> Option<String> {
    let path = announced_path()?;
    std::fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
}

fn remember(version: &str) {
    let Some(path) = announced_path() else {
        return;
    };
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(%error, "could not create the config directory");
        return;
    }
    // Failing here costs one repeated notification at the next launch and
    // nothing else, so it is a warning rather than anything louder.
    if let Err(error) = std::fs::write(&path, version) {
        tracing::warn!(%error, path = %path.display(), "could not record the announced version");
    }
}

/// Turn the plugin's errors into something a person can act on.
///
/// The two that matter are told apart: a build with no update channel is a
/// packaging mistake, and a server that will not answer is Tuesday.
fn describe(error: tauri_plugin_updater::Error) -> String {
    let text = error.to_string();

    if text.contains("public key") || text.contains("pubkey") || text.contains("not configured") {
        return "This build has no update channel configured, so it cannot check for updates. \
                Download the newest version from the site instead."
            .to_owned();
    }

    format!("Could not reach the update server: {text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_notes_are_left_alone() {
        assert_eq!(shorten("Two words.", 180), "Two words.");
    }

    #[test]
    fn long_notes_end_on_a_word() {
        let notes = "Recognition accuracy is now a setting, and this sentence carries on well past the limit so that the cut has somewhere to land.";
        let short = shorten(notes, 40);
        assert!(short.ends_with('…'), "{short}");
        assert!(short.chars().count() <= 41, "{short}");
        assert!(!short.contains(" …"), "cut mid-space: {short}");
        assert!(
            notes.starts_with(short.trim_end_matches(['…', ' '])),
            "{short}"
        );
    }

    /// The notes are UTF-8 from a manifest we do not control. Counting bytes
    /// rather than characters would panic on a cut through a multi-byte one,
    /// inside the startup task, in a background app.
    #[test]
    fn a_cut_through_multibyte_text_does_not_panic() {
        let notes =
            "Настройки — Голос — Распознавание выбирает между быстрым и точным режимом работы";
        let short = shorten(notes, 20);
        assert!(short.chars().count() <= 21, "{short}");
    }

    #[test]
    fn whitespace_only_notes_shorten_to_nothing_rather_than_panicking() {
        assert_eq!(shorten("   \n  ", 180), "");
    }
}
