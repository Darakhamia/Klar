//! Fetching models from the interface rather than from a terminal.
//!
//! `klar_core::model::download_to` does the work — resuming, hashing, refusing
//! a corrupt file. This is the part that turns its progress into events and
//! stops the same model being fetched twice at once.

use klar_core::model::{self, ModelSpec, Progress};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashSet;
use tauri::{AppHandle, Emitter, Manager};

/// The channel onboarding's progress bar listens to. Separate from the engine's
/// channel: a download runs while the pipeline is dead, and the two have
/// nothing to say to each other.
pub const EVENT: &str = "klar://model";

/// Which models are being fetched right now, so a second click is a no-op
/// rather than two writers on one `.part` file.
#[derive(Default)]
pub struct Active(Mutex<HashSet<String>>);

impl Active {
    /// Claim `id`, or report that it is already being fetched.
    fn claim(&self, id: &str) -> bool {
        self.0.lock().insert(id.to_owned())
    }

    fn release(&self, id: &str) {
        self.0.lock().remove(id);
    }
}

/// Where a download has got to, as the interface sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "phase")]
pub enum DownloadEvent {
    Downloading {
        id: String,
        received: u64,
        total: u64,
    },
    /// Hashing the finished file. On a slow disk this is not instant, so it
    /// says so rather than looking like a hang at 100%.
    Verifying {
        id: String,
    },
    Done {
        id: String,
    },
    Failed {
        id: String,
        message: String,
    },
}

fn emit(app: &AppHandle, event: DownloadEvent) {
    if let Err(error) = app.emit(EVENT, &event) {
        tracing::warn!(%error, "could not deliver download progress to the interface");
    }
}

/// Start fetching `spec` in the background. Returns as soon as the task is
/// queued; everything else arrives on [`EVENT`].
pub fn start(app: &AppHandle, active: &Active, spec: &'static ModelSpec) {
    if !active.claim(spec.id) {
        tracing::debug!(id = spec.id, "already downloading");
        return;
    }

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let outcome = fetch(&handle, spec).await;

        if let Some(active) = handle.try_state::<Active>() {
            active.release(spec.id);
        }

        match outcome {
            Ok(()) => emit(
                &handle,
                DownloadEvent::Done {
                    id: spec.id.to_owned(),
                },
            ),
            Err(message) => {
                tracing::error!(id = spec.id, %message, "model download failed");
                emit(
                    &handle,
                    DownloadEvent::Failed {
                        id: spec.id.to_owned(),
                        message,
                    },
                );
            }
        }
    });
}

async fn fetch(app: &AppHandle, spec: &'static ModelSpec) -> Result<(), String> {
    let dir = model::models_dir().ok_or("could not find the app data directory")?;
    let handle = app.clone();

    model::download_to(spec, &dir, move |progress| {
        let event = match progress {
            Progress::Resuming { from, total } => DownloadEvent::Downloading {
                id: spec.id.to_owned(),
                received: from,
                total,
            },
            Progress::Downloading { received, total } => DownloadEvent::Downloading {
                id: spec.id.to_owned(),
                received,
                total,
            },
            Progress::Verifying => DownloadEvent::Verifying {
                id: spec.id.to_owned(),
            },
            // `Done` is emitted by the caller, once, after the task has also
            // released its claim — otherwise the interface can ask for a
            // restart while this model still counts as busy.
            Progress::Done => return,
        };
        emit(&handle, event);
    })
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_is_only_fetched_once_at_a_time() {
        let active = Active::default();

        assert!(active.claim("large-v3-turbo-q5_0"));
        assert!(
            !active.claim("large-v3-turbo-q5_0"),
            "a second click must not put two writers on one .part file"
        );
        assert!(
            active.claim("silero-vad"),
            "a different model is not blocked by the first"
        );

        active.release("large-v3-turbo-q5_0");
        assert!(
            active.claim("large-v3-turbo-q5_0"),
            "released, so a retry can start"
        );
    }
}
