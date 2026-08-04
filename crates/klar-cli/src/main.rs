//! Headless harness for the pipeline.
//!
//! Everything risky gets spiked here before it goes near a window. If a feature
//! cannot be exercised from this binary, it is in the wrong crate.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use klar_core::asr::{Backend, TranscribeOptions, Transcriber, WhisperTranscriber};
use klar_core::audio::{self, Capture, CaptureConfig};
use klar_core::model::{self, Progress};
use klar_core::{Input, Machine};
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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("KLAR_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match Cli::parse().command.unwrap_or(Command::Version) {
        Command::Version => version(),
        Command::Doctor => doctor(),
        Command::Devices => devices(),
        Command::Record(args) => record(&args),
        Command::Transcribe(args) => transcribe(&args),
        Command::Listen(args) => listen(&args),
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
