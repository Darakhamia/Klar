//! Headless harness for the pipeline.
//!
//! Everything risky gets spiked here before it goes near a window. If a feature
//! cannot be exercised from this binary, it is in the wrong crate.

use anyhow::Result;
use clap::{Parser, Subcommand};
use klar_core::{Input, Machine};

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
    /// Report what the OS says about each permission Klar needs.
    Doctor,
    /// Drive the state machine through one dictation with canned text.
    ///
    /// Real audio arrives in M1; this exists so the event stream the overlay
    /// subscribes to can be watched before there is an overlay.
    DryRun,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("KLAR_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match Cli::parse().command.unwrap_or(Command::Version) {
        Command::Version => version(),
        Command::Doctor => doctor(),
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
    Ok(())
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
