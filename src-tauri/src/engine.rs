//! The dictation engine: everything `klar-cli dictate` does, running inside the
//! app and reporting itself to the interface.
//!
//! It owns the models, the hotkey and the injector, and runs on its own thread.
//! The frontend never drives it — it only listens. Every state change is an
//! event, and the overlay renders from those and holds no state of its own.

use crate::settings::FinishAction;
use klar_core::asr::{TranscribeOptions, WhisperTranscriber};
use klar_core::audio::{self, BlockConverter, Capture, CaptureConfig};
use klar_core::polish::{OllamaConfig, PolishRequest, Polisher, Strength, TextPolisher};
use klar_core::store::{NewDictation, Store, StoreError};
use klar_core::stream::{Stream, StreamConfig, Update};
use klar_core::vad::{StreamingVad, Vad, VadSettings};
use klar_core::{Input, Machine, State, StateEvent, model};
use klar_platform::{Binding, HotkeyEvent};
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
    /// Everything is loaded and the hotkey is live. Onboarding waits on this
    /// before offering its test field.
    Ready { hotkey: Binding },
}

/// Settings the engine reads.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub model: String,
    pub language: Option<String>,
    pub device: Option<String>,
    pub hotkey: Binding,
    pub finish: FinishAction,
    pub cleanup: Strength,
    pub accuracy: klar_core::asr::Accuracy,
    /// `None` leaves the transcript alone — see `Settings::polish`.
    pub polish: Option<OllamaConfig>,
    pub stream: StreamConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            model: model::DEFAULT_MODEL.to_owned(),
            language: None,
            device: None,
            hotkey: klar_platform::default_binding(),
            finish: FinishAction::default(),
            cleanup: Strength::default(),
            accuracy: klar_core::asr::Accuracy::default(),
            polish: None,
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
            hotkey: settings.hotkey.clone(),
            finish: settings.on_finish,
            cleanup: settings.cleanup,
            accuracy: settings.accuracy,
            polish: settings.polish(),
            stream: StreamConfig::default(),
        }
    }
}

/// Handle to the running engine. Dropping it stops the hotkey and waits for it.
pub struct Engine {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
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
            thread: Some(thread),
        }
    }

    /// Stop, and wait for the keyboard hook to actually come down.
    ///
    /// Waiting matters: the caller's next move is to install another hook, and
    /// for the moment both are registered they both match the same key. When
    /// the caller is about to capture a new binding, the old hook winning that
    /// race means pressing the key to rebind it starts a dictation instead.
    ///
    /// It costs the length of one poll — the engine checks between hotkey
    /// waits, 100 ms apart — and the engine thread never waits on the caller,
    /// so this cannot deadlock against the main thread.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("the dictation engine thread panicked");
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shutdown();
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
        // Onboarding downloads these and restarts the engine. Said here rather
        // than left to fail at the first hotkey press, which would look like
        // the hotkey not working.
        return Err("the speech model is not downloaded yet — finish setup to fetch it".into());
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

    // The polish stage is HTTP, and the rest of this thread is not. One
    // current-thread runtime, built here and used for nothing else, keeps the
    // async confined to the one call that needs it — and this thread is not
    // inside a runtime, so blocking on it cannot nest.
    let mut polisher = match &config.polish {
        Some(ollama) => match klar_core::polish::Ollama::new(ollama.clone()) {
            Ok(polisher) => {
                warm(ollama.clone());
                Polisher::Ollama(polisher)
            }
            Err(error) => {
                tracing::warn!(%error, "polish is configured but unusable; dictating plain");
                Polisher::Noop
            }
        },
        None => Polisher::Noop,
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("could not start the polish runtime: {error}"))?;

    let binding = config.hotkey.clone();
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
            hotkey: binding.clone(),
        },
    );
    emit(app, UiEvent::State { state: State::Idle });

    // Its own connection rather than sharing the one the settings window uses:
    // SQLite in WAL mode is built for exactly this, and it keeps a history
    // query in the interface from ever being in the way of the write at the end
    // of a dictation.
    //
    // A database that will not open costs the dictionary and the history, and
    // nothing else. Dictation is the product; both of those are features of it.
    let mut store = match klar_core::store::default_path() {
        Some(path) => match Store::open(&path) {
            Ok(store) => Some(store),
            Err(error) => {
                tracing::error!(%error, "no database: the dictionary and history are unavailable");
                None
            }
        },
        None => {
            tracing::error!("no app data directory: the dictionary and history are unavailable");
            None
        }
    };

    while !stop.load(Ordering::SeqCst) {
        match hotkey_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(HotkeyEvent::Pressed) => {
                let outcome = dictate(
                    app,
                    config,
                    store.as_mut(),
                    &mut transcriber,
                    &mut vad,
                    &mut polisher,
                    &runtime,
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
    store: Option<&mut Store>,
    transcriber: &mut WhisperTranscriber,
    vad: &mut StreamingVad,
    polisher: &mut Polisher,
    runtime: &tokio::runtime::Runtime,
    injector: &mut dyn klar_platform::TextInjector,
    hotkey_rx: &Receiver<HotkeyEvent>,
    stop: &AtomicBool,
) -> Result<(), String> {
    let mut machine = Machine::new();
    apply(app, &mut machine, Input::Start);

    // Read per dictation, not once at startup: a word taught in the settings
    // window has to work on the very next sentence, which is the only moment
    // the user will think to test it. A few rows against a pipeline that is
    // about to run an encoder pass.
    let dictionary = store
        .as_ref()
        .map(|store| store.dictionary())
        .transpose()
        .unwrap_or_else(|error: StoreError| {
            tracing::warn!(%error, "could not read the dictionary");
            None
        })
        .unwrap_or_default();

    let options = TranscribeOptions {
        language: config.language.clone(),
        // Biases recognition toward the taught words before anything is
        // decoded; `Dictionary::apply` below fixes what this misses.
        initial_prompt: dictionary.prompt().map(|(prompt, _)| prompt),
        accuracy: config.accuracy,
        ..TranscribeOptions::default()
    };

    let capture_config = CaptureConfig {
        device: config.device.clone(),
    };
    let (capture, audio_rx) = Capture::start(&capture_config).map_err(|e| e.to_string())?;
    let mut converter = BlockConverter::new(capture.format(), Duration::from_millis(250));
    let mut stream = Stream::new(transcriber, vad, options, config.stream);

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

    // The clock the latency budget is measured against starts here.
    let released = Instant::now();
    apply(app, &mut machine, Input::Stop);

    let tail = converter.flush().map_err(|e| e.to_string())?;
    if !tail.is_empty() {
        stream.push(&tail).map_err(|e| e.to_string())?;
    }
    capture.stop();

    let (raw, stats) = stream.finish().map_err(|e| e.to_string())?;
    if raw.is_empty() {
        // Nothing said. Not an error, and not worth an overlay full of nothing.
        apply(app, &mut machine, Input::Fail("no speech".into()));
        apply(app, &mut machine, Input::Dismiss);
        return Ok(());
    }

    // Before polish rather than after: a language model handed a mangled name
    // will confidently tidy it into a different mangled name, and the polish
    // prompts forbid inventing words the speaker did not say. Correct the name
    // first and the model sees what was meant.
    let text = dictionary.apply(&raw);

    apply(app, &mut machine, Input::Transcribed(text.clone()));
    emit(
        app,
        UiEvent::Text {
            text: text.clone(),
            settled: true,
        },
    );

    let started_polish = Instant::now();
    let polished = polish(runtime, polisher, &text, config.cleanup);
    let polish_ms = started_polish.elapsed().as_millis() as u64;

    apply(app, &mut machine, Input::Polished(polished.clone()));
    if polished != text {
        emit(
            app,
            UiEvent::Text {
                text: polished.clone(),
                settled: true,
            },
        );
    }

    // Asked before injecting: the paste is about to change what has focus in
    // some applications, and the answer wanted is where the text was aimed.
    let target = klar_platform::foreground_app();

    let delivered = deliver(&polished, config.finish, injector).map_err(|e| e.to_string())?;
    let latency_ms = released.elapsed().as_millis() as u64;

    tracing::info!(
        method = ?delivered,
        finish = ?config.finish,
        cleanup = ?config.cleanup,
        commits = stats.commits,
        tail_ms = stats.tail_elapsed.as_millis() as u64,
        polish_ms,
        latency_ms,
        target = target.as_deref().unwrap_or("-"),
        "dictation delivered"
    );
    apply(app, &mut machine, Input::Injected);

    // Last, and never fatal. The words are already in the user's document; a
    // history row that did not save is worth a line in the log and nothing
    // more.
    if let Some(store) = store {
        let recorded = store.record(&NewDictation {
            text: polished,
            raw,
            audio_ms: (stats.committed_audio + stats.tail_audio).as_millis() as u64,
            latency_ms,
            target,
            polished: config.cleanup != Strength::Verbatim,
        });
        if let Err(error) = recorded {
            tracing::warn!(%error, "could not record the dictation");
        }
    }

    Ok(())
}

/// Load the polish model into memory, in the background, now.
///
/// A cold Ollama takes tens of seconds to load a model — 25 seconds measured
/// for a 3B one, against a budget in hundreds of milliseconds. Somebody is going to pay that, and it
/// should be the app at startup rather than the user at their first sentence.
///
/// Its own thread with its own runtime: this must not hold up registering the
/// hotkey, and a failure is not worth reporting to anybody — the dictation path
/// already falls back to the transcript.
fn warm(config: OllamaConfig) {
    let spawned = std::thread::Builder::new()
        .name("klar-polish-warm".into())
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            let Ok(ollama) = klar_core::polish::Ollama::new(config) else {
                return;
            };

            match runtime.block_on(ollama.warm()) {
                Ok(took) => tracing::info!(
                    took_ms = took.as_millis() as u64,
                    "polish model loaded and held in memory"
                ),
                Err(error) => tracing::warn!(%error, "could not warm the polish model"),
            }
        });

    if let Err(error) = spawned {
        tracing::warn!(%error, "could not start the warm-up thread");
    }
}

/// Run the polish stage, or hand the transcript back unchanged.
///
/// Every failure here returns the transcript. A model that is down, slow, or
/// answering instead of editing must not cost the user the words they just
/// said — plain text now beats polished text never, and the log says which
/// happened.
fn polish(
    runtime: &tokio::runtime::Runtime,
    polisher: &mut Polisher,
    text: &str,
    cleanup: Strength,
) -> String {
    if cleanup == Strength::Verbatim || matches!(polisher, Polisher::Noop) {
        return text.to_owned();
    }

    let request = PolishRequest {
        text,
        strength: cleanup,
        // M5's dictionary fills this in.
        vocabulary: &[],
        language: None,
    };

    match runtime.block_on(polisher.polish(request)) {
        Ok(polished) => polished,
        Err(error) => {
            tracing::warn!(%error, "polish failed; inserting the transcript as recognised");
            text.to_owned()
        }
    }
}

/// Put the finished text wherever the user asked for it.
///
/// `None` for the copy-only path: nothing was injected, so there is no method
/// to report.
fn deliver(
    text: &str,
    finish: FinishAction,
    injector: &mut dyn klar_platform::TextInjector,
) -> Result<Option<klar_platform::InjectionMethod>, klar_platform::PlatformError> {
    match finish {
        FinishAction::Type => injector.inject(text).map(Some),
        FinishAction::Copy => klar_platform::copy_to_clipboard(text).map(|()| None),
        // Not `inject` followed by a copy: the restore that injection schedules
        // runs on a timer after it returns, so a copy made in between is wiped
        // 150 ms later. The injector has to be told to keep the text instead.
        FinishAction::TypeAndCopy => injector.inject_and_keep(text).map(Some),
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
