//! What the polish stage must and must not do to real dictations.
//!
//! These run against a live model, so they are skipped unless one is pointed at.
//! Either the model Klar ships, through its own sidecar:
//!
//! ```sh
//! cargo build -p klar-llm --features engine
//! KLAR_POLISH_GGUF=~/models/qwen2.5-1.5b-instruct-q4_k_m.gguf \
//! KLAR_POLISH_SIDECAR=target/debug/klar-llm \
//!   cargo test -p klar-core --test polish_fixtures -- --nocapture
//! ```
//!
//! or a model server the machine already runs:
//!
//! ```sh
//! KLAR_OLLAMA_MODEL=llama3.2:3b cargo test -p klar-core --test polish_fixtures
//! ```
//!
//! Skipped rather than failed when the variable is absent: CI has no GPU and no
//! model server, and a red build there would say nothing about the code.
//!
//! The assertions are properties rather than exact strings. A language model
//! will not produce the same sentence twice across versions, and a test that
//! demanded it would be rewritten every time a model was updated until somebody
//! deleted it. What must hold is narrower and permanent: the fillers go, the
//! facts stay, and the model never answers what it was given.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "an integration test; a failure here is the report"
)]

use klar_core::polish::{Ollama, OllamaConfig, PolishRequest, Strength, TextPolisher, sidecar};
use std::time::{Duration, Instant};

/// One dictation and what must be true of the result.
struct Fixture {
    /// What the speaker said, as whisper would hand it over.
    said: &'static str,
    strength: Strength,
    /// Words that must survive. Facts, names, numbers, decisions.
    keeps: &'static [&'static str],
    /// Words that must be gone. Fillers, and the halves of self-corrections
    /// the speaker abandoned.
    drops: &'static [&'static str],
    /// What whisper would have reported. Passed through exactly as the pipeline
    /// passes it, because for a small model it is the difference between a
    /// cleanup and a translation — see `PolishRequest::language`.
    language: Option<&'static str>,
}

impl Fixture {
    /// Everything wrong with one result, as sentences a person can act on.
    fn faults_in(&self, polished: &str) -> Vec<String> {
        let lowered = polished.to_lowercase();
        let mut faults = Vec::new();

        for keep in self.keeps {
            if !lowered.contains(&keep.to_lowercase()) {
                faults.push(format!(
                    "lost {keep:?}\n  said: {}\n  got:  {polished}",
                    self.said
                ));
            }
        }
        for drop in self.drops {
            if lowered.contains(&drop.to_lowercase()) {
                faults.push(format!(
                    "kept {drop:?}\n  said: {}\n  got:  {polished}",
                    self.said
                ));
            }
        }
        faults
    }
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        said: "so I guess we should um push the review to Thursday no Friday \
               and uh I'll write up the notes after",
        strength: Strength::Balanced,
        keeps: &["Friday", "notes"],
        // The abandoned half of the correction, and the fillers around it.
        drops: &["Thursday", "um", "uh"],
        language: Some("en"),
    },
    Fixture {
        said: "can you send me the numbers for Q3 when you get a chance",
        strength: Strength::Balanced,
        // The one that matters: this is a question, and the model must clean it
        // rather than answer it. `drops` catches the shapes an answer takes.
        keeps: &["Q3"],
        drops: &["Sure", "Certainly", "I don't have", "As an AI"],
        language: Some("en"),
    },
    Fixture {
        said: "the deploy is at four thirty and Marcus is on call and the rollback \
               plan is in the runbook so we should be fine",
        strength: Strength::Balanced,
        keeps: &["Marcus", "runbook", "rollback"],
        drops: &[],
        language: Some("en"),
    },
    Fixture {
        said: "so basically what I'm trying to say is that at the end of the day \
               we should probably just go ahead and push the release back by about \
               a week or so",
        strength: Strength::Heavy,
        keeps: &["week"],
        drops: &["basically", "at the end of the day", "I'm trying to say"],
        language: Some("en"),
    },
    Fixture {
        said: "перенесём ревью на четверг нет на пятницу и я потом напишу заметки",
        strength: Strength::Balanced,
        keeps: &["пятниц", "заметки"],
        drops: &["Friday", "review"],
        language: Some("ru"),
    },
    // Code-switching: started in one language, finished in another. Open, and
    // here so that choosing a polish model is judged against it rather than
    // around it.
    //
    // No model tried so far can do this. Qwen2.5-1.5B translates the whole
    // thing into English whether the prompt names one language, names the mix,
    // or says nothing — four attempts, four translations. Qwen2.5-3B keeps the
    // Russian and drops the first half of the sentence instead. The guard
    // refuses all of it (47%, 69% and 36% word overlap against a 75% floor), so
    // what reaches the user is the transcript unpolished rather than a
    // translation — the right failure, but a failure.
    //
    // `language: None` because there is no single answer whisper could give
    // that would be true, and naming either half is what makes the model
    // translate the other.
    Fixture {
        said: "so um let's move the review to Friday и я потом напишу заметки и отправлю Марку",
        strength: Strength::Balanced,
        keeps: &["Friday", "заметки", "Марку"],
        drops: &["um", "notes", "send"],
        language: None,
    },
];

/// The sidecar path: Klar's own model, in the process an install runs it in.
#[tokio::test]
async fn the_fixtures_survive_the_model_klar_ships() {
    let Ok(gguf) = std::env::var("KLAR_POLISH_GGUF") else {
        eprintln!("skipped: set KLAR_POLISH_GGUF to a GGUF to run this");
        return;
    };

    let program = match std::env::var("KLAR_POLISH_SIDECAR") {
        Ok(path) => std::path::PathBuf::from(path),
        Err(_) => sidecar::beside_current_exe()
            .expect("no klar-llm beside the test binary; set KLAR_POLISH_SIDECAR"),
    };

    let mut sidecar = sidecar::Sidecar::start(sidecar::SidecarConfig {
        program,
        model: std::path::PathBuf::from(&gguf),
        // Not the pipeline's 400 ms. These say whether the prompt and the guard
        // are right, and the machine running them may have no GPU at all;
        // latency is reported below rather than asserted.
        budget: Duration::from_secs(120),
        ..sidecar::SidecarConfig::default()
    })
    .await
    .expect("the sidecar starts");

    println!(
        "{} on {} in {} ms\n",
        sidecar.ready().model,
        sidecar.ready().backend,
        sidecar.ready().load_ms
    );

    let mut failures = Vec::new();
    for fixture in FIXTURES {
        let started = Instant::now();
        let polished = sidecar
            .polish(PolishRequest {
                text: fixture.said,
                strength: fixture.strength,
                vocabulary: &[],
                language: fixture.language,
            })
            .await;
        let took = started.elapsed();

        match polished {
            Ok(text) => {
                failures.extend(fixture.faults_in(&text));
                println!("{took:>8?}  {text}");
            }
            Err(error) => failures.push(format!(
                "{:?}\n  said: {}\n  {error}",
                fixture.strength, fixture.said
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} fixtures failed against {gguf}:\n\n{}",
        failures.len(),
        FIXTURES.len(),
        failures.join("\n\n")
    );
}

#[tokio::test]
async fn the_fixtures_survive_a_real_model() {
    let Ok(model) = std::env::var("KLAR_OLLAMA_MODEL") else {
        eprintln!("skipped: set KLAR_OLLAMA_MODEL to the name of a model this Ollama has pulled");
        return;
    };

    let endpoint = std::env::var("KLAR_OLLAMA_ENDPOINT")
        .unwrap_or_else(|_| klar_core::polish::ollama::DEFAULT_ENDPOINT.to_owned());

    let mut ollama = Ollama::new(OllamaConfig {
        endpoint: endpoint.clone(),
        model: model.clone(),
        // Not the pipeline's 400 ms: these run on whatever machine happens to
        // have a model, and a slow one failing here would say nothing useful.
        // Latency is measured below and reported rather than asserted.
        budget: Duration::from_secs(30),
        ..OllamaConfig::default()
    })
    .expect("client builds");

    assert!(
        ollama.available().await,
        "no model server at {endpoint} — start Ollama or unset KLAR_OLLAMA_MODEL"
    );

    // Loading the model is tens of seconds and is not what these measure. The
    // first run without this had two fixtures fail on a timeout that was the
    // disk, not the prompt.
    match ollama.warm().await {
        Ok(took) => println!("model loaded in {took:?}\n"),
        Err(error) => panic!("could not load {model}: {error}"),
    }

    let mut failures = Vec::new();
    let mut slowest = Duration::ZERO;

    for fixture in FIXTURES {
        let started = Instant::now();
        let polished = ollama
            .polish(PolishRequest {
                text: fixture.said,
                strength: fixture.strength,
                vocabulary: &[],
                language: None,
            })
            .await;
        let took = started.elapsed();
        slowest = slowest.max(took);

        let polished = match polished {
            Ok(text) => text,
            Err(error) => {
                failures.push(format!(
                    "{:?}\n  said: {}\n  {error}",
                    fixture.strength, fixture.said
                ));
                continue;
            }
        };

        failures.extend(fixture.faults_in(&polished));

        println!("{:>8?}  {polished}", took);
    }

    println!("\nslowest: {slowest:?} against {model}");
    assert!(
        failures.is_empty(),
        "{} of {} fixtures failed:\n\n{}",
        failures.len(),
        FIXTURES.len(),
        failures.join("\n\n")
    );
}
