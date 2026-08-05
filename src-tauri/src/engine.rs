//! The dictation engine: everything `klar-cli dictate` does, running inside the
//! app and reporting itself to the interface.
//!
//! It owns the models, the hotkey and the injector, and runs on its own thread.
//! The frontend never drives it — it only listens. Every state change is an
//! event, and the overlay renders from those and holds no state of its own.

use klar_core::asr::{TranscribeOptions, WhisperTranscriber};
use klar_core::audio::{self, BlockConverter, Capture, CaptureConfig};
use klar_core::stream::{Stream, StreamConfig, Update};
use klar_core::vad::{StreamingVad, Vad, VadSettings};
use klar_core::{Input, Machine, State, StateEvent, model};
use klar_platform::HotkeyEvent;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// The event name the frontend subscribes to. One channel for everything, so
/// the overlay never has to correlate two streams.
pub const EVENT: &str = "klar://event";

/// What the interface is told. Mirrors the core state machine, plus the two
/// things only the running pipeline knows: the input level and the text.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum UiEvent {
    /// The pipeline moved. The overlay's five visual states come from here.
    State { state: State },
    /// Peak input level, 0..1, while recording. Drives the waveform.
    Level { peak: f32 },
    /// Recognised text. `settled` is false while it can still change.
    Text { text: String, settled: bool },
    /// Something went wrong, written for a person.
    Failed { message: String },
    /// Models are still loading; the hotkey will not do anything yet.
    Loading { what: String },
    /// Everything is loaded and the hotkey is live.
    Ready { hotkey: String },
}

/// Settings the engine reads. Written by the settings window in a later slice;
/// for now these are the defaults the CLI proved out.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub model: String,
    pub language: Option<String>,
    pub device: Option<String>,
    pub stream: StreamConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            model: model::DEFAULT_MODEL.to_owned(),
            language: None,
            device: None,
            stream: StreamConfig::default(),
        }
    }
}

impl From<&crate::settings::Settings> for EngineConfig {
    fn from(settings: &crate::settings::Settings) -> Self {
        Self {
            model: settings.model.clone(),
            language: settings.language.clone(),
            device: settings.microphone.clone(),
            stream: StreamConfig::default(),
        }
    }
}

/// Handle to the running engine. Dropping it stops the hotkey.
pub struct Engine {
    stop: Arc<AtomicBool>,
    _thread: std::thread::JoinHandle<()>,
}

impl Engine {
    /// Start the engine in the background.
    ///
    /// Returns immediately: the models take the best part of a second to load,
    /// and the tray has to appear before that.
    pub fn start(app: AppHandle, config: EngineConfig) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_in_thread = Arc::clone(&stop);

        let thread = std::thread::Builder::new()
            .name("klar-engine".into())
            .spawn(move || {
                if let Err(error) = run(&app, &config, &stop_in_thread) {
                    tracing::error!(%error, "the dictation engine stopped");
                    emit(&app, UiEvent::Failed { message: error });
                }
            })
            .unwrap_or_else(|error| {
                // Spawning the engine thread is the one failure with nothing
                // left to run; there is no app without it.
                panic!("could not start the dictation engine: {error}");
            });

        Self {
            stop,
            _thread: thread,
        }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

fn emit(app: &AppHandle, event: UiEvent) {
    if let Err(error) = app.emit(EVENT, &event) {
        tracing::warn!(%error, "could not deliver an event to the interface");
    }
}

/// Load everything, then wait on the hotkey until told to stop.
fn run(app: &AppHandle, config: &EngineConfig, stop: &AtomicBool) -> Result<(), String> {
    let dir = model::models_dir().ok_or("could not find the app data directory")?;

    let spec =
        model::find(&config.model).ok_or_else(|| format!("unknown model {}", config.model))?;
    let vad_spec =
        model::find(model::VAD_MODEL).ok_or("the vad model is missing from the catalogue")?;

    let model_path = model::path_in(&dir, spec);
    let vad_path = model::path_in(&dir, vad_spec);

    if !model_path.is_file() || !vad_path.is_file() {
        // Onboarding downloads these. Until it exists, say plainly what is
        // missing rather than failing at the first hotkey press.
        return Err(format!(
            "models are not installed yet — run `klar-cli model download` and `klar-cli model download {}`",
            vad_spec.id
        ));
    }

    emit(
        app,
        UiEvent::Loading {
            what: spec.id.to_owned(),
        },
    );
    let mut transcriber = WhisperTranscriber::load(&model_path).map_err(|e| e.to_string())?;
    let mut vad =
        StreamingVad::new(Vad::load(&vad_path, VadSettings::default()).map_err(|e| e.to_string())?);
    let mut injector = klar_platform::injector();

    let binding = klar_platform::default_binding();
    let (hotkey_tx, hotkey_rx) = channel();
    let mut hotkey = klar_platform::hotkey();
    hotkey
        .register(
            &binding,
            Box::new(move |event| {
                let _ = hotkey_tx.send(event);
            }),
        )
        .map_err(|e| e.to_string())?;

    emit(
        app,
        UiEvent::Ready {
            hotkey: format!("{binding:?}"),
        },
    );
    emit(app, UiEvent::State { state: State::Idle });

    let options = TranscribeOptions {
        language: config.language.clone(),
        ..TranscribeOptions::default()
    };

    while !stop.load(Ordering::SeqCst) {
        match hotkey_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(HotkeyEvent::Pressed) => {
                let outcome = dictate(
                    app,
                    config,
                    &options,
                    &mut transcriber,
                    &mut vad,
                    &mut *injector,
                    &hotkey_rx,
                    stop,
                );
                if let Err(message) = outcome {
                    tracing::error!(%message, "dictation failed");
                    emit(
                        app,
                        UiEvent::State {
                            state: State::Error,
                        },
                    );
                    emit(app, UiEvent::Failed { message });
                }
            }
            Ok(HotkeyEvent::Released) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = hotkey.unregister();
    Ok(())
}

/// One dictation, from key down to the text landing.
#[allow(clippy::too_many_arguments)]
fn dictate(
    app: &AppHandle,
    config: &EngineConfig,
    options: &TranscribeOptions,
    transcriber: &mut WhisperTranscriber,
    vad: &mut StreamingVad,
    injector: &mut dyn klar_platform::TextInjector,
    hotkey_rx: &Receiver<HotkeyEvent>,
    stop: &AtomicBool,
) -> Result<(), String> {
    let mut machine = Machine::new();
    apply(app, &mut machine, Input::Start);

    let capture_config = CaptureConfig {
        device: config.device.clone(),
    };
    let (capture, audio_rx) = Capture::start(&capture_config).map_err(|e| e.to_string())?;
    let mut converter = BlockConverter::new(capture.format(), Duration::from_millis(250));
    let mut stream = Stream::new(transcriber, vad, options.clone(), config.stream);

    let mut raw = Vec::new();
    let mut level_at = Instant::now();

    loop {
        raw.clear();
        if audio::capture::drain(&audio_rx, &mut raw).is_none() {
            return Err("the microphone stopped".into());
        }

        if !raw.is_empty() {
            // The waveform wants a steady pulse, not one per callback.
            if level_at.elapsed() >= Duration::from_millis(50) {
                level_at = Instant::now();
                emit(
                    app,
                    UiEvent::Level {
                        peak: audio::peak(&raw),
                    },
                );
            }

            let block = converter.push(&raw).map_err(|e| e.to_string())?;
            if !block.is_empty()
                && let Some(update) = stream.push(&block).map_err(|e| e.to_string())?
            {
                let (text, settled) = match update {
                    Update::Partial(text) => (text, false),
                    Update::Committed(text) => (text, true),
                };
                emit(
                    app,
                    UiEvent::Text {
                        text,
                        settled: false,
                    },
                );
                let _ = settled;
            }
        }

        if stop.load(Ordering::SeqCst) {
            break;
        }

        match hotkey_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(HotkeyEvent::Released) => break,
            Ok(HotkeyEvent::Pressed) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    apply(app, &mut machine, Input::Stop);

    let tail = converter.flush().map_err(|e| e.to_string())?;
    if !tail.is_empty() {
        stream.push(&tail).map_err(|e| e.to_string())?;
    }
    capture.stop();

    let (text, stats) = stream.finish().map_err(|e| e.to_string())?;
    if text.is_empty() {
        // Nothing said. Not an error, and not worth an overlay full of nothing.
        apply(app, &mut machine, Input::Fail("no speech".into()));
        apply(app, &mut machine, Input::Dismiss);
        return Ok(());
    }

    apply(app, &mut machine, Input::Transcribed(text.clone()));
    emit(
        app,
        UiEvent::Text {
            text: text.clone(),
            settled: true,
        },
    );

    // M4 puts the polish stage here. Until then the transcript goes straight
    // through, and the state machine passes through Polishing untouched.
    apply(app, &mut machine, Input::Polished(text.clone()));

    let injected = injector.inject(&text);
    match injected {
        Ok(method) => {
            tracing::info!(
                ?method,
                commits = stats.commits,
                tail_ms = stats.tail_elapsed.as_millis() as u64,
                "dictation delivered"
            );
            apply(app, &mut machine, Input::Injected);
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Drive the state machine and forward whatever it emits.
fn apply(app: &AppHandle, machine: &mut Machine, input: Input) {
    match machine.apply(input) {
        Ok(events) => {
            for event in events {
                match event {
                    StateEvent::Entered(state) => emit(app, UiEvent::State { state }),
                    StateEvent::Partial(text) => {
                        emit(
                            app,
                            UiEvent::Text {
                                text,
                                settled: false,
                            },
                        );
                    }
                    StateEvent::Final(text) => emit(
                        app,
                        UiEvent::Text {
                            text,
                            settled: true,
                        },
                    ),
                    StateEvent::Failed(message) => emit(app, UiEvent::Failed { message }),
                }
            }
        }
        Err(error) => {
            // An unexpected ordering is worth a log and nothing more; the
            // dictation carries on rather than the app falling over.
            tracing::warn!(%error, "illegal state transition");
        }
    }
}
