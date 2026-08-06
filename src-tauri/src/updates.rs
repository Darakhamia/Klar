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
use tauri::AppHandle;
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
/// Its own task so it cannot delay the tray, the window or the engine, and it
/// reports what it finds by changing the tray tooltip — the quietest place that
/// is always visible. A failure is logged and nothing else: somebody starting a
/// dictation app has not asked to hear about the network.
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
                crate::tray::announce_update(&handle, &available.version);
            }
            Ok(None) => tracing::info!("klar is up to date"),
            Err(error) => tracing::info!(%error, "could not check for updates"),
        }
    });
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
