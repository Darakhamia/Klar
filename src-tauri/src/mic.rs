//! The microphone test in onboarding's first step.
//!
//! Step one has one question to answer — can Klar hear you — and on Windows
//! there is no permission API that answers it: a desktop app is not blocked by
//! the Privacy toggle the way a packaged one is, so `permission_state` says
//! `Unknown` and means it. Opening the device is the only honest answer, so
//! this opens a real capture stream and emits the same level events the
//! overlay's waveform already reads.
//!
//! Nothing here writes audio anywhere. The samples are reduced to one peak
//! value per 50 ms and dropped.
//!
//! No unit test: every line of this is opening a real capture device, and a
//! machine with no microphone — CI, for one — cannot exercise it. The
//! conversion and level maths it leans on are tested in `klar-core`.

use crate::engine::{EVENT, UiEvent};
use klar_core::audio::{self, Capture, CaptureConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// How often the level is reported. The waveform wants a steady pulse, not one
/// event per capture callback.
const TICK: Duration = Duration::from_millis(50);

/// A running microphone test. Dropping it closes the stream.
pub struct MicTest {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MicTest {
    /// Open the device and start reporting levels.
    ///
    /// Fails if the device cannot be opened, which is exactly the case
    /// onboarding needs to show the user.
    pub fn start(app: AppHandle, device: Option<String>) -> Result<Self, String> {
        let (capture, audio_rx) =
            Capture::start(&CaptureConfig { device }).map_err(|e| e.to_string())?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_in_thread = Arc::clone(&stop);

        let thread = std::thread::Builder::new()
            .name("klar-mic-test".into())
            .spawn(move || {
                let mut raw = Vec::new();
                while !stop_in_thread.load(Ordering::Relaxed) {
                    raw.clear();
                    if audio::capture::drain(&audio_rx, &mut raw).is_none() {
                        tracing::warn!("the microphone stopped during the test");
                        break;
                    }
                    if !raw.is_empty()
                        && let Err(error) = app.emit(
                            EVENT,
                            UiEvent::Level {
                                peak: audio::peak(&raw),
                            },
                        )
                    {
                        tracing::warn!(%error, "could not deliver a level to the interface");
                    }
                    std::thread::sleep(TICK);
                }
                capture.stop();
            })
            .map_err(|e| e.to_string())?;

        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for MicTest {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("the microphone test thread panicked");
        }
    }
}
