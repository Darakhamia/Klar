//! Headless harness for the pipeline.
//!
//! Everything risky gets spiked here before it goes near a window. If a feature
//! cannot be exercised from this binary, it is in the wrong crate.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use klar_core::asr::{Backend, TranscribeOptions, Transcriber, WhisperTranscriber};
use klar_core::audio::{self, Capture, CaptureConfig};
use klar_core::model::{self, Progress};
use klar_core::stream::{Stream, StreamConfig, StreamStats, Update};
use klar_core::vad::{StreamingVad, Vad, VadSettings};
use klar_core::{Input, Machine};
use klar_platform::{HotkeyEvent, InjectionMethod};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "klar-cli",
    version,
    about = "Headless harness for the Klar pipeline"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print the version and the platform Klar thinks it is running on.
    Version,
    /// Report what the OS, the audio stack and the ASR build all say.
    Doctor,
    /// List the microphones Klar can see.
    Devices,
    /// Capture from the microphone and report what arrived.
    Record(RecordArgs),
    /// Transcribe a wav file. No microphone needed — this is the fixture path.
    Transcribe(TranscribeArgs),
    /// Record, then transcribe. The M1 acceptance criterion end to end.
    Listen(ListenArgs),
    /// Type text into whatever window is focused. No microphone involved.
    Inject(InjectArgs),
    /// Watch the push-to-talk hotkey and print its events.
    Hotkey,
    /// Hold the hotkey, speak, release, and the text lands in the focused app.
    /// The M2 acceptance criterion end to end.
    Dictate(DictateArgs),
    /// Show what the voice activity detector finds in a wav file.
    Vad(VadArgs),
    /// Manage the whisper models.
    #[command(subcommand)]
    Model(ModelCommand),
    /// Drive the state machine through one dictation with canned text.
    DryRun,
}

#[derive(clap::Args)]
struct RecordArgs {
    /// How long to capture.
    #[arg(long, default_value_t = 5.0)]
    seconds: f32,
    /// Input device name; the system default if omitted.
    #[arg(long)]
    device: Option<String>,
    /// Write the captured audio to a wav file.
    ///
    /// Audio never otherwise touches the disk — this is the explicit debug path
    /// and it only happens because you asked on the command line.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(clap::Args)]
struct TranscribeArgs {
    /// A wav file at any rate; it is resampled to 16 kHz mono on the way in.
    file: PathBuf,
    #[arg(long, default_value = model::DEFAULT_MODEL)]
    model: String,
    /// ISO language code. Omit to let whisper detect it.
    #[arg(long)]
    language: Option<String>,
}

#[derive(clap::Args)]
struct ListenArgs {
    #[arg(long, default_value_t = 5.0)]
    seconds: f32,
    #[arg(long)]
    device: Option<String>,
    #[arg(long, default_value = model::DEFAULT_MODEL)]
    model: String,
    #[arg(long)]
    language: Option<String>,
}

#[derive(clap::Args)]
struct VadArgs {
    file: PathBuf,
    /// Silero's probability threshold, 0..1.
    #[arg(long, default_value_t = 0.5)]
    threshold: f32,
    /// Silence shorter than this is a gap within a sentence, in milliseconds.
    #[arg(long, default_value_t = 400)]
    min_silence: u64,
    /// Also run the incremental detector the streaming pass uses, feeding the
    /// file through in chunks, and compare the two.
    #[arg(long)]
    stream: bool,
}

#[derive(clap::Args)]
struct InjectArgs {
    /// The text to insert.
    text: String,
    /// clipboard (default) or keystrokes.
    #[arg(long, default_value = "clipboard")]
    method: Method,
    /// Seconds to wait before injecting, so you can focus the target window.
    #[arg(long, default_value_t = 3.0)]
    delay: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Method {
    Clipboard,
    Keystrokes,
}

impl From<Method> for InjectionMethod {
    fn from(method: Method) -> Self {
        match method {
            Method::Clipboard => Self::Clipboard,
            Method::Keystrokes => Self::Keystrokes,
        }
    }
}

#[derive(clap::Args)]
struct DictateArgs {
    #[arg(long)]
    device: Option<String>,
    #[arg(long, default_value = model::DEFAULT_MODEL)]
    model: String,
    #[arg(long)]
    language: Option<String>,
    /// Longest utterance to accept, as a guard against a stuck key.
    #[arg(long, default_value_t = 60.0)]
    max_seconds: f32,
    /// Transcribe but do not inject. For checking recognition without a target.
    #[arg(long)]
    dry: bool,
}

#[derive(Subcommand)]
enum ModelCommand {
    /// Show the catalogue and what is installed.
    List,
    /// Download a model, resuming if a partial file is there.
    Download {
        #[arg(default_value = model::DEFAULT_MODEL)]
        id: String,
    },
    /// Re-hash an installed model and compare it against the catalogue.
    Verify {
        #[arg(default_value = model::DEFAULT_MODEL)]
        id: String,
    },
    /// Print the models directory.
    Path,
}

/// whisper.cpp narrates every VAD call at info level, which buries everything
/// else once streaming starts. Its own logs are demoted; ggml's are not,
/// because `ggml_cuda_init: found 1 CUDA devices` is the only runtime proof
/// that the GPU was actually picked up. Raise it with
/// `KLAR_LOG=info,whisper_rs=info` when that is what you are looking at.
const DEFAULT_LOG: &str = "info,whisper_rs::whisper_logging_hook=warn";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("KLAR_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_LOG)),
        )
        .init();

    match Cli::parse().command.unwrap_or(Command::Version) {
        Command::Version => version(),
        Command::Doctor => doctor(),
        Command::Devices => devices(),
        Command::Record(args) => record(&args),
        Command::Transcribe(args) => transcribe(&args),
        Command::Listen(args) => listen(&args),
        Command::Vad(args) => vad(&args),
        Command::Inject(args) => inject(&args),
        Command::Hotkey => hotkey(),
        Command::Dictate(args) => dictate(&args),
        Command::Model(command) => model_command(command).await,
        Command::DryRun => dry_run(),
    }
}

fn version() -> Result<()> {
    println!(
        "klar {} ({} {})",
        klar_core::VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    Ok(())
}

fn doctor() -> Result<()> {
    use klar_platform::Permission;

    println!("klar {}", klar_core::VERSION);
    println!(
        "platform      {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("hotkey        {:?}", klar_platform::default_binding());
    for permission in [Permission::Microphone, Permission::Accessibility] {
        println!(
            "{permission:<13} {:?}",
            klar_platform::permission_state(permission)
        );
    }

    let backend = Backend::compiled();
    println!("asr backend   {backend}");
    if !backend.is_gpu() {
        println!(
            "              ^ CPU build. Rebuild with --features cuda (Windows) or metal (macOS)."
        );
    }
    println!("whisper.cpp   {}", klar_core::asr::whisper::system_info());

    match audio::capture_devices() {
        Ok(devices) if devices.is_empty() => println!("microphones   none found"),
        Ok(devices) => {
            for device in devices {
                let marker = if device.is_default { "*" } else { " " };
                println!("microphone  {marker} {}", device.name);
            }
        }
        Err(error) => println!("microphones   unavailable: {error}"),
    }

    let dir = models_dir()?;
    println!("models        {}", dir.display());
    for spec in model::CATALOGUE {
        let state = if model::is_present(&dir, spec) {
            "installed"
        } else {
            "-"
        };
        println!("  {:<22} {:>9}  {}", spec.id, spec.human_size(), state);
    }

    Ok(())
}

fn devices() -> Result<()> {
    let devices = audio::capture_devices().context("listing input devices")?;
    if devices.is_empty() {
        bail!("no input devices found");
    }
    for device in devices {
        println!(
            "{} {}",
            if device.is_default { "*" } else { " " },
            device.name
        );
    }
    Ok(())
}

fn record(args: &RecordArgs) -> Result<()> {
    let samples = capture(args.seconds, args.device.as_deref())?;

    let seconds = samples.len() as f32 / audio::SAMPLE_RATE as f32;
    println!(
        "captured      {:.2} s  ({} samples at {} Hz)",
        seconds,
        samples.len(),
        audio::SAMPLE_RATE
    );
    println!("peak          {:.3}", audio::peak(&samples));
    println!("rms           {:.4}", audio::rms(&samples));

    if audio::peak(&samples) < 0.001 {
        println!("\nThe microphone produced silence. Check the input device and its level.");
    }

    if let Some(path) = &args.out {
        audio::wav::write_mono(path, &samples).context("writing the debug wav")?;
        println!("wrote         {}", path.display());
    }

    Ok(())
}

fn transcribe(args: &TranscribeArgs) -> Result<()> {
    let samples = audio::wav::read_as_whisper_input(&args.file)
        .with_context(|| format!("reading {}", args.file.display()))?;
    run_asr(&samples, &args.model, args.language.as_deref())
}

fn listen(args: &ListenArgs) -> Result<()> {
    // Record first, then load the model. Fine for a fixed-length CLI capture;
    // in the app it would be backwards, because a model load after the user has
    // started talking loses the start of the sentence. M3 keeps the model
    // resident for exactly that reason.
    let samples = capture(args.seconds, args.device.as_deref())?;
    run_asr(&samples, &args.model, args.language.as_deref())
}

/// Capture `seconds` of audio and hand it back as 16 kHz mono.
fn capture(seconds: f32, device: Option<&str>) -> Result<Vec<f32>> {
    if !(0.1..=600.0).contains(&seconds) {
        bail!("--seconds must be between 0.1 and 600");
    }

    let config = CaptureConfig {
        device: device.map(str::to_owned),
    };
    let (capture, rx) = Capture::start(&config).context("opening the microphone")?;
    let format = capture.format();
    let capture_name = capture.device_name().to_owned();

    println!(
        "recording     {} — {} Hz, {} ch, {:.1} s",
        capture.device_name(),
        format.sample_rate,
        format.channels,
        seconds
    );

    println!("\nSPEAK NOW");

    let deadline = Instant::now() + Duration::from_secs_f32(seconds);
    let mut raw = Vec::new();
    let mut meter_at = Instant::now();

    while Instant::now() < deadline {
        let before = raw.len();
        if audio::capture::drain(&rx, &mut raw).is_none() {
            bail!("the capture stream stopped early");
        }

        // A live level meter, not a row of dots. A silent take is the most
        // likely reason a test dictation comes back as nonsense, and it has to
        // be obvious while it is happening rather than afterwards.
        if meter_at.elapsed() >= Duration::from_millis(100) {
            meter_at = Instant::now();
            let remaining = deadline.saturating_duration_since(Instant::now());
            print!(
                "\r{}  {:>4.1}s ",
                meter(audio::peak(&raw[before..])),
                remaining.as_secs_f32()
            );
            std::io::stdout().flush().ok();
        }

        std::thread::sleep(Duration::from_millis(20));
    }
    // One last pass: the callback may have delivered a block while we slept.
    audio::capture::drain(&rx, &mut raw);
    capture.stop();
    println!("\r{:60}", "");

    if raw.is_empty() {
        bail!("the microphone delivered no audio at all");
    }

    let samples = audio::to_whisper_input(&raw, format).context("converting to 16 kHz mono")?;

    // Whisper does not return an empty string for silence — it invents a
    // plausible sentence, sometimes in the wrong alphabet entirely. Refusing
    // here turns a confusing result into a clear one.
    let peak = audio::peak(&samples);
    if peak < SILENCE_PEAK {
        bail!(
            "the microphone recorded silence (peak {peak:.4}). \n\
             Check that '{}' is the right device and is not muted — \n\
             `klar-cli devices` lists the alternatives.",
            capture_name
        );
    }

    Ok(samples)
}

/// Below this, there is no speech in the buffer — only the noise floor.
const SILENCE_PEAK: f32 = 0.01;

/// A twenty-cell bar. Peak is scaled with a square root so quiet speech still
/// moves it; this is a "is anything arriving" indicator, not a VU meter.
fn meter(peak: f32) -> String {
    const CELLS: usize = 20;
    let filled = (peak.sqrt() * CELLS as f32).round().min(CELLS as f32) as usize;
    let mark = if peak < SILENCE_PEAK { ' ' } else { '#' };
    format!(
        "[{}{}]",
        String::from_iter(std::iter::repeat_n(mark, filled)),
        " ".repeat(CELLS - filled)
    )
}

fn run_asr(samples: &[f32], model_id: &str, language: Option<&str>) -> Result<()> {
    let spec = model::find(model_id)
        .with_context(|| format!("unknown model '{model_id}' — try `klar-cli model list`"))?;
    let dir = models_dir()?;
    let path = model::path_in(&dir, spec);

    if !path.is_file() {
        bail!(
            "{} is not installed — run `klar-cli model download {}`",
            spec.id,
            spec.id
        );
    }

    let loading = Instant::now();
    let mut transcriber = WhisperTranscriber::load(&path)?;
    let load = loading.elapsed();

    let options = TranscribeOptions {
        language: language.map(str::to_owned),
        ..TranscribeOptions::default()
    };
    let transcript = transcriber.transcribe(samples, &options)?;

    println!();
    println!("{}", transcript.text);
    println!();
    println!("backend       {}", transcriber.backend());
    println!("model load    {} ms", load.as_millis());
    println!("audio         {:.2} s", transcript.audio.as_secs_f32());
    println!("transcription {} ms", transcript.elapsed.as_millis());
    println!("realtime      {:.1}x", transcript.real_time_factor());
    if let Some(language) = &transcript.language {
        println!("language      {language}");
    }

    Ok(())
}

/// Report the speech the detector finds, and what trimming would save.
fn vad(args: &VadArgs) -> Result<()> {
    let samples = audio::wav::read_as_whisper_input(&args.file)
        .with_context(|| format!("reading {}", args.file.display()))?;

    let dir = models_dir()?;
    let spec = model::find(model::VAD_MODEL).context("the vad model is not in the catalogue")?;
    let path = model::path_in(&dir, spec);
    if !path.is_file() {
        bail!("run `klar-cli model download {}` first", spec.id);
    }

    let settings = VadSettings {
        threshold: args.threshold,
        min_silence: Duration::from_millis(args.min_silence),
        ..VadSettings::default()
    };

    let started = Instant::now();
    let mut vad = Vad::load(&path, settings)?;
    let load = started.elapsed();

    let started = Instant::now();
    let segments = vad.segments(&samples)?;
    let elapsed = started.elapsed();

    let total = audio::SAMPLE_RATE as f32;
    println!("audio         {:.2} s", samples.len() as f32 / total);
    println!("vad load      {} ms", load.as_millis());
    println!("vad run       {} ms", elapsed.as_millis());
    println!("segments      {}", segments.len());

    let mut speech = 0_usize;
    for (index, segment) in segments.iter().enumerate() {
        speech += segment.len();
        println!(
            "  {index:>2}  {:>6.2} → {:>6.2} s   ({:.2} s)",
            segment.start as f32 / total,
            segment.end as f32 / total,
            segment.duration().as_secs_f32()
        );
    }

    if samples.is_empty() {
        return Ok(());
    }
    let kept = speech as f32 / samples.len() as f32 * 100.0;
    println!(
        "speech        {kept:.0}% of the buffer — trimming saves {:.0}%",
        100.0 - kept
    );

    if args.stream {
        compare_streaming(vad, &samples, &segments)?;
    }
    Ok(())
}

/// Feed the same audio through the incremental detector in capture-sized
/// chunks and show what it found.
///
/// The streaming path analyses each second once and segments from stored
/// probabilities; the one-shot path re-runs the model over everything. They
/// should agree, and this is where that is checked against real speech rather
/// than a synthetic probability track.
fn compare_streaming(
    vad: Vad,
    samples: &[f32],
    one_shot: &[klar_core::vad::Segment],
) -> Result<()> {
    use klar_core::vad::StreamingVad;

    let mut streaming = StreamingVad::new(vad);
    let started = Instant::now();

    // 20 ms at a time, as the capture callback delivers.
    let chunk = audio::SAMPLE_RATE as usize / 50;
    let mut fed = 0;
    while fed < samples.len() {
        let end = (fed + chunk).min(samples.len());
        streaming.advance(&samples[..end])?;
        fed = end;
    }

    let found = streaming.segments();
    let total = audio::SAMPLE_RATE as f32;

    println!();
    println!(
        "streaming     {} ms for the whole file, in 20 ms pushes",
        started.elapsed().as_millis()
    );
    println!(
        "segments      {} (one-shot found {})",
        found.len(),
        one_shot.len()
    );
    for (index, segment) in found.iter().enumerate() {
        println!(
            "  {index:>2}  {:>6.2} → {:>6.2} s",
            segment.start as f32 / total,
            segment.end as f32 / total
        );
    }
    Ok(())
}

/// Insert text into the focused window. The delay exists because the terminal
/// is focused when you press Enter, and typing into it proves nothing.
fn inject(args: &InjectArgs) -> Result<()> {
    let method: InjectionMethod = args.method.into();
    println!("focus the target window — injecting in {:.0}s", args.delay);
    for remaining in (1..=args.delay.ceil() as u32).rev() {
        print!("\r{remaining} ");
        std::io::stdout().flush().ok();
        std::thread::sleep(Duration::from_secs(1));
    }
    println!("\r      ");

    let started = Instant::now();
    let used = klar_platform::injector().inject_using(&args.text, method)?;
    println!(
        "injected via {used:?} in {} ms",
        started.elapsed().as_millis()
    );
    Ok(())
}

/// Print hotkey events until interrupted. The quickest way to tell whether the
/// low-level hook is installed and whether both edges arrive.
fn hotkey() -> Result<()> {
    let binding = klar_platform::default_binding();
    let (tx, rx) = std::sync::mpsc::channel();

    let mut hotkey = klar_platform::hotkey();
    hotkey.register(
        &binding,
        Box::new(move |event| {
            let _ = tx.send(event);
        }),
    )?;

    println!("hold {binding:?} anywhere. Ctrl+C to stop.");

    let mut pressed_at: Option<Instant> = None;
    for event in rx {
        match event {
            HotkeyEvent::Pressed => {
                pressed_at = Some(Instant::now());
                println!("down");
            }
            HotkeyEvent::Released => {
                let held = pressed_at.take().map_or(0, |at| at.elapsed().as_millis());
                println!("up    (held {held} ms)");
            }
        }
    }
    Ok(())
}

/// The whole thing: hold the hotkey, speak, release, text appears.
///
/// Transcription runs while the user is still talking, so key release only
/// leaves the tail — the last phrase since their final pause.
fn dictate(args: &DictateArgs) -> Result<()> {
    let dir = models_dir()?;

    let spec = model::find(&args.model)
        .with_context(|| format!("unknown model '{}' — try `klar-cli model list`", args.model))?;
    let path = model::path_in(&dir, spec);
    if !path.is_file() {
        bail!(
            "{} is not installed — run `klar-cli model download {}`",
            spec.id,
            spec.id
        );
    }

    let vad_spec =
        model::find(model::VAD_MODEL).context("the vad model is not in the catalogue")?;
    let vad_path = model::path_in(&dir, vad_spec);
    if !vad_path.is_file() {
        bail!("run `klar-cli model download {}` first", vad_spec.id);
    }

    // Both models load once and stay resident. Loading either after the user
    // has started speaking would lose the beginning of the sentence.
    println!("loading {} ...", spec.id);
    let mut transcriber = WhisperTranscriber::load(&path)?;
    let mut vad = StreamingVad::new(Vad::load(&vad_path, VadSettings::default())?);
    let mut injector = klar_platform::injector();

    let binding = klar_platform::default_binding();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut hotkey = klar_platform::hotkey();
    hotkey.register(
        &binding,
        Box::new(move |event| {
            let _ = tx.send(event);
        }),
    )?;

    println!("ready. hold {binding:?}, speak, release. Ctrl+C to stop.");

    let options = TranscribeOptions {
        language: args.language.clone(),
        ..TranscribeOptions::default()
    };
    let config = StreamConfig::default();
    let mut timings = Timings::default();

    loop {
        // Idle until the key goes down. A stray release here is nothing.
        match rx.recv() {
            Ok(HotkeyEvent::Pressed) => {}
            Ok(HotkeyEvent::Released) => continue,
            Err(_) => break,
        }

        let capture_config = CaptureConfig {
            device: args.device.as_deref().map(str::to_owned),
        };
        let (capture, audio_rx) = match Capture::start(&capture_config) {
            Ok(started) => started,
            Err(error) => {
                println!("could not open the microphone: {error}");
                continue;
            }
        };

        println!("\nlistening ...");
        let mut converter =
            audio::BlockConverter::new(capture.format(), Duration::from_millis(250));
        let mut stream = Stream::new(&mut transcriber, &mut vad, options.clone(), config);
        let started = Instant::now();
        let mut raw = Vec::new();
        let mut shown = String::new();

        // Recognise as it arrives, until the key comes up.
        let released = loop {
            raw.clear();
            if audio::capture::drain(&audio_rx, &mut raw).is_none() {
                println!("capture stopped unexpectedly");
                break None;
            }
            if !raw.is_empty() {
                let block = converter.push(&raw)?;
                if !block.is_empty()
                    && let Some(update) = stream.push(&block)?
                {
                    let text = match &update {
                        Update::Partial(text) | Update::Committed(text) => text,
                    };
                    if *text != shown {
                        shown.clone_from(text);
                        print!("\r{:<78}", truncate(text, 78));
                        std::io::stdout().flush().ok();
                    }
                }
            }

            if started.elapsed().as_secs_f32() > args.max_seconds {
                println!("\nhit the {:.0}s limit; stopping", args.max_seconds);
                break Some(Instant::now());
            }

            match rx.recv_timeout(Duration::from_millis(20)) {
                Ok(HotkeyEvent::Released) => break Some(Instant::now()),
                Ok(HotkeyEvent::Pressed) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break None,
            }
        };

        // Anything the converter was still holding belongs to this dictation.
        let tail = converter.flush()?;
        if !tail.is_empty() {
            stream.push(&tail)?;
        }
        capture.stop();

        let (text, stats) = stream.finish()?;
        print!("\r{:<78}\r", "");

        let Some(released) = released else { break };

        if text.is_empty() {
            println!("no speech recognised");
            continue;
        }

        println!("{text}");

        if args.dry {
            report(&stats, None, released, &mut timings);
            continue;
        }

        let inject_started = Instant::now();
        let method = injector.inject(&text)?;
        report(
            &stats,
            Some((method, inject_started.elapsed())),
            released,
            &mut timings,
        );
    }

    Ok(())
}

/// Key-up-to-text, one entry per dictation.
///
/// M3's criterion is a median over at least twenty real dictations, so the
/// numbers accumulate here rather than being read off the screen one at a time.
#[derive(Default)]
struct Timings {
    key_up_to_text: Vec<u128>,
}

impl Timings {
    fn record(&mut self, millis: u128) {
        self.key_up_to_text.push(millis);
    }

    fn summary(&self) -> String {
        if self.key_up_to_text.is_empty() {
            return String::new();
        }
        let mut sorted = self.key_up_to_text.clone();
        sorted.sort_unstable();
        let median = sorted[sorted.len() / 2];
        // The worst case is what the user remembers, so it is worth showing
        // alongside the median the criterion is written against.
        let worst = sorted.last().copied().unwrap_or(median);
        format!(
            "   [{} dictations: median {median} ms, worst {worst} ms]",
            sorted.len()
        )
    }
}

/// Cut a partial to fit one terminal line without splitting a character.
fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    // Keep the end: that is where the newest words are.
    let skip = text.chars().count() - width + 1;
    format!("…{}", text.chars().skip(skip).collect::<String>())
}

fn report(
    stats: &StreamStats,
    injected: Option<(InjectionMethod, Duration)>,
    released: Instant,
    timings: &mut Timings,
) {
    let key_up_to_text = released.elapsed().as_millis();
    timings.record(key_up_to_text);

    let injection = match injected {
        Some((method, elapsed)) => format!("inject {} ms ({method:?})   ", elapsed.as_millis()),
        None => "not injected   ".to_owned(),
    };

    println!(
        "  streamed {:.1}s in {} commit(s)   tail {:.1}s in {} ms   {injection}key-up to text {key_up_to_text} ms{}",
        stats.committed_audio.as_secs_f32(),
        stats.commits,
        stats.tail_audio.as_secs_f32(),
        stats.tail_elapsed.as_millis(),
        timings.summary()
    );
}

async fn model_command(command: ModelCommand) -> Result<()> {
    let dir = models_dir()?;

    match command {
        ModelCommand::Path => {
            println!("{}", dir.display());
        }

        ModelCommand::List => {
            for spec in model::CATALOGUE {
                let state = if model::is_present(&dir, spec) {
                    "installed"
                } else {
                    "-"
                };
                let default = if spec.id == model::DEFAULT_MODEL {
                    " (default)"
                } else {
                    ""
                };
                println!(
                    "{:<22} {:>9}  {:<10}{}",
                    spec.id,
                    spec.human_size(),
                    state,
                    default
                );
                println!("  {}", spec.summary);
            }
        }

        ModelCommand::Download { id } => {
            let spec = model::find(&id)
                .with_context(|| format!("unknown model '{id}' — try `klar-cli model list`"))?;
            println!("{} → {}", spec.id, dir.display());

            let mut last_percent = u64::MAX;
            let path = model::download_to(spec, &dir, |progress| match progress {
                Progress::Resuming { from, total } => {
                    println!("resuming at {} of {} bytes", from, total);
                }
                Progress::Downloading { received, total } => {
                    let percent = received * 100 / total.max(1);
                    if percent != last_percent {
                        last_percent = percent;
                        print!("\r{percent:>3}%  {received} / {total} bytes");
                        std::io::stdout().flush().ok();
                    }
                }
                Progress::Verifying => println!("\nverifying checksum"),
                Progress::Done => println!("done"),
            })
            .await?;

            println!("{}", path.display());
        }

        ModelCommand::Verify { id } => {
            let spec = model::find(&id)
                .with_context(|| format!("unknown model '{id}' — try `klar-cli model list`"))?;
            if model::download::verify(spec, &dir)? {
                println!("{}: checksum matches", spec.id);
            } else {
                bail!("{}: checksum does NOT match — re-download it", spec.id);
            }
        }
    }

    Ok(())
}

fn models_dir() -> Result<PathBuf> {
    // An override keeps tests and CI out of the real app data dir.
    if let Ok(dir) = std::env::var("KLAR_MODELS_DIR") {
        return Ok(PathBuf::from(dir));
    }
    model::models_dir().context("could not determine the app data directory")
}

fn dry_run() -> Result<()> {
    let script = [
        Input::Start,
        Input::Partial("so the thing is um".into()),
        Input::Stop,
        Input::Transcribed("so the thing is um we ship on tuesday no friday".into()),
        Input::Polished("We ship on Friday.".into()),
        Input::Injected,
    ];

    let mut machine = Machine::new();
    for input in script {
        for event in machine.apply(input)? {
            println!("{event:?}");
        }
    }
    println!("final state: {}", machine.state());
    Ok(())
}
