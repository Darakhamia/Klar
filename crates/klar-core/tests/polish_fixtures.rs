//! What the polish stage must and must not do to real dictations.
//!
//! These run against a live model, so they are skipped unless one is pointed at:
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
    reason = "an integration test; a failure here is the report"
)]

use klar_core::polish::{Ollama, OllamaConfig, PolishRequest, Strength, TextPolisher};
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
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        said: "so I guess we should um push the review to Thursday no Friday \
               and uh I'll write up the notes after",
        strength: Strength::Balanced,
        keeps: &["Friday", "notes"],
        // The abandoned half of the correction, and the fillers around it.
        drops: &["Thursday", "um", "uh"],
    },
    Fixture {
        said: "can you send me the numbers for Q3 when you get a chance",
        strength: Strength::Balanced,
        // The one that matters: this is a question, and the model must clean it
        // rather than answer it. `drops` catches the shapes an answer takes.
        keeps: &["Q3"],
        drops: &["Sure", "Certainly", "I don't have", "As an AI"],
    },
    Fixture {
        said: "the deploy is at four thirty and Marcus is on call and the rollback \
               plan is in the runbook so we should be fine",
        strength: Strength::Balanced,
        keeps: &["Marcus", "runbook", "rollback"],
        drops: &[],
    },
    Fixture {
        said: "so basically what I'm trying to say is that at the end of the day \
               we should probably just go ahead and push the release back by about \
               a week or so",
        strength: Strength::Heavy,
        keeps: &["week"],
        drops: &["basically", "at the end of the day", "I'm trying to say"],
    },
    Fixture {
        said: "перенесём ревью на четверг нет на пятницу и я потом напишу заметки",
        strength: Strength::Balanced,
        // Must come back in Russian, not translated into the prompt's language.
        keeps: &["пятниц", "заметки"],
        drops: &["Friday", "review"],
    },
];

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
    })
    .expect("client builds");

    assert!(
        ollama.available().await,
        "no model server at {endpoint} — start Ollama or unset KLAR_OLLAMA_MODEL"
    );

    let mut failures = Vec::new();
    let mut slowest = Duration::ZERO;

    for fixture in FIXTURES {
        let started = Instant::now();
        let polished = ollama
            .polish(PolishRequest {
                text: fixture.said,
                strength: fixture.strength,
                vocabulary: &[],
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

        let lowered = polished.to_lowercase();
        for keep in fixture.keeps {
            if !lowered.contains(&keep.to_lowercase()) {
                failures.push(format!(
                    "lost {keep:?}\n  said: {}\n  got:  {polished}",
                    fixture.said
                ));
            }
        }
        for drop in fixture.drops {
            if lowered.contains(&drop.to_lowercase()) {
                failures.push(format!(
                    "kept {drop:?}\n  said: {}\n  got:  {polished}",
                    fixture.said
                ));
            }
        }

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
