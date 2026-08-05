//! Turning a transcript into finished text.
//!
//! This is the stage that makes Klar not a transcriber. Speech that has been
//! recognised correctly is still speech: it has fillers, false starts, and
//! corrections the speaker made mid-sentence and expects you to have followed.
//!
//! The whole risk of the stage is in one sentence, and it is worth stating
//! before any of the code: **a language model asked to tidy up a paragraph will
//! sometimes answer it instead.** Dictate "should we move the review to Friday"
//! and a model that has drifted will hand back "Yes, Friday works well." That is
//! not a worse cleanup, it is a different thing entirely, and it would be typed
//! into whatever the user had open. The prompts say so at length and
//! [`guard`] refuses the ones that do it anyway.
//!
//! Nothing here is on by default beyond [`Strength::Verbatim`], which does not
//! call a model at all.

pub mod ollama;

pub use ollama::{Ollama, OllamaConfig};

use std::time::Duration;

/// How much the polisher may rewrite.
///
/// The prompts live in `prompts/polish/v1/` rather than in string literals:
/// they are the behaviour of this stage, they will be edited far more often
/// than this file, and a diff of a prompt should read like a diff of prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Strength {
    /// Off. No model is called and no text leaves this machine — see
    /// [`Strength::prompt`].
    Verbatim,
    Light,
    #[default]
    Balanced,
    Heavy,
}

impl Strength {
    /// The instructions for this strength, or `None` for the one that means
    /// "do not run this stage".
    ///
    /// `None` is not an empty prompt. The caller must not construct a polisher
    /// at all: the point of verbatim is that the transcript never reaches a
    /// language model, local or otherwise.
    pub const fn prompt(self) -> Option<&'static str> {
        match self {
            Self::Verbatim => None,
            Self::Light => Some(include_str!("../../prompts/polish/v1/light.md")),
            Self::Balanced => Some(include_str!("../../prompts/polish/v1/balanced.md")),
            Self::Heavy => Some(include_str!("../../prompts/polish/v1/heavy.md")),
        }
    }

    /// How much shorter or longer the result may be than the transcript,
    /// as a fraction of the transcript's length.
    ///
    /// A cleanup removes fillers, so it shrinks; the floor is where shrinking
    /// stops being a cleanup and starts being a summary. The ceiling is lower
    /// than it looks — punctuation adds almost nothing, so anything much longer
    /// is the model having written something of its own.
    const fn bounds(self) -> (f32, f32) {
        match self {
            // Never used: verbatim does not call a model.
            Self::Verbatim => (1.0, 1.0),
            Self::Light => (0.7, 1.25),
            Self::Balanced => (0.55, 1.25),
            // Heavy buys its rewriting with a weaker guard, and the floor was
            // measured rather than picked: "so basically what I'm trying to say
            // is that at the end of the day we should probably just go ahead
            // and push the release back by about a week or so" is a quarter of
            // its length once the padding is gone, and that is a cleanup the
            // user asked for, not a summary.
            Self::Heavy => (0.24, 1.15),
        }
    }

    /// How much of the result must be words the speaker actually said.
    ///
    /// The signal length cannot give. A cleanup is the speaker's own words with
    /// the fillers taken out, so nearly every word of it appears in the
    /// transcript. An answer is not: "I don't have any Q3 numbers to send"
    /// shares three words with the question it was asked, and that is the
    /// failure this stage exists to catch — it went past the length guard at
    /// 67% and had to be caught by a person reading the output.
    ///
    /// Not 100%, because a cleanup legitimately rewrites some words: "four
    /// thirty" becomes "4:30", "I will" becomes "I'll". Heavy is lowest because
    /// rewriting sentences is what it was asked to do.
    const fn min_overlap(self) -> f32 {
        match self {
            Self::Verbatim => 1.0,
            Self::Light => 0.85,
            Self::Balanced => 0.75,
            Self::Heavy => 0.55,
        }
    }
}

/// Why a polished result was thrown away and the transcript used instead.
///
/// Kept as a type rather than a string: each of these is a different way for
/// the model to have gone wrong, and the log wants to say which.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum Rejection {
    #[error("the model returned nothing")]
    Empty,

    #[error("the result is {ratio:.0}% of the transcript, below the {floor:.0}% floor")]
    TooShort { ratio: f32, floor: f32 },

    #[error("the result is {ratio:.0}% of the transcript, above the {ceiling:.0}% ceiling")]
    TooLong { ratio: f32, ceiling: f32 },

    #[error("only {overlap:.0}% of the result is words the speaker said, below {floor:.0}%")]
    Invented { overlap: f32, floor: f32 },
}

#[derive(Debug, thiserror::Error)]
pub enum PolishError {
    #[error("{0} is not reachable — is the local model server running?")]
    Unreachable(String),

    #[error("the polish service answered {status}: {body}")]
    Refused { status: u16, body: String },

    #[error("the polish service sent something that is not a reply: {0}")]
    Unreadable(String),

    #[error("polish took longer than {0:?}")]
    TooSlow(Duration),

    #[error("{0}")]
    Rejected(#[from] Rejection),
}

/// Everything a polisher needs to know for one call.
#[derive(Debug, Clone)]
pub struct PolishRequest<'a> {
    pub text: &'a str,
    pub strength: Strength,
    /// Terms the user has taught Klar, so the model does not helpfully correct
    /// a name it has never seen. Empty until M5 fills the dictionary in.
    pub vocabulary: &'a [String],
}

/// A stage that turns a transcript into finished text.
///
/// Async because every implementation but one is an HTTP call, and the one that
/// is not returns instantly. The engine owns a small runtime to drive it; the
/// CLI is already async.
#[allow(async_fn_in_trait, reason = "no implementation is used behind dyn")]
pub trait TextPolisher: Send {
    async fn polish(&mut self, request: PolishRequest<'_>) -> Result<String, PolishError>;

    /// Whether the service would answer right now. Used by the settings window
    /// to say so before the user finds out mid-dictation.
    async fn available(&mut self) -> bool;
}

/// The polisher the engine holds.
///
/// An enum rather than `Box<dyn TextPolisher>`: there are three implementations,
/// all known at compile time, and a trait object would cost an allocation and a
/// boxed future to say the same thing.
pub enum Polisher {
    /// Verbatim. Hands the transcript straight back and touches nothing.
    Noop,
    Ollama(Ollama),
}

impl TextPolisher for Polisher {
    async fn polish(&mut self, request: PolishRequest<'_>) -> Result<String, PolishError> {
        match self {
            Self::Noop => Ok(request.text.to_owned()),
            Self::Ollama(ollama) => ollama.polish(request).await,
        }
    }

    async fn available(&mut self) -> bool {
        match self {
            Self::Noop => true,
            Self::Ollama(ollama) => ollama.available().await,
        }
    }
}

/// Decide whether a polished result is a cleaned-up transcript or something
/// else the model decided to write.
///
/// Length is a crude signal and deliberately so. It cannot tell a good cleanup
/// from a mediocre one, and it is not trying to: it is there to catch the
/// failure that matters, where the model answers or summarises instead of
/// tidying, and those miss by a mile rather than by a word.
pub fn guard(transcript: &str, polished: &str, strength: Strength) -> Result<String, Rejection> {
    let polished = polished.trim();
    if polished.is_empty() {
        return Err(Rejection::Empty);
    }

    // Characters rather than bytes: a Cyrillic transcript is twice the bytes of
    // the same text in Latin, and the ratio must not depend on the language.
    let before = transcript.trim().chars().count();
    if before == 0 {
        return Ok(polished.to_owned());
    }

    let ratio = polished.chars().count() as f32 / before as f32;
    let (floor, ceiling) = strength.bounds();

    if ratio < floor {
        return Err(Rejection::TooShort {
            ratio: ratio * 100.0,
            floor: floor * 100.0,
        });
    }
    if ratio > ceiling {
        return Err(Rejection::TooLong {
            ratio: ratio * 100.0,
            ceiling: ceiling * 100.0,
        });
    }

    let overlap = overlap(transcript, polished);
    let floor = strength.min_overlap();
    if overlap < floor {
        return Err(Rejection::Invented {
            overlap: overlap * 100.0,
            floor: floor * 100.0,
        });
    }

    Ok(polished.to_owned())
}

/// The fraction of the polished text's words that the speaker also said.
///
/// Word by word rather than in order: a cleanup is allowed to reorder a clause
/// and to drop the halves of a self-correction, and neither is the failure this
/// is looking for.
fn overlap(transcript: &str, polished: &str) -> f32 {
    let said: std::collections::HashSet<String> = words(transcript).collect();
    if said.is_empty() {
        return 1.0;
    }

    let mut total = 0_u32;
    let mut kept = 0_u32;
    for word in words(polished) {
        total += 1;
        if said.contains(&word) {
            kept += 1;
        }
    }

    if total == 0 {
        return 1.0;
    }
    f32::from(u16::try_from(kept).unwrap_or(u16::MAX))
        / f32::from(u16::try_from(total).unwrap_or(u16::MAX))
}

/// Lowercased runs of letters and digits. Punctuation is what the polisher was
/// asked to add, so it cannot count against it.
fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_has_no_prompt_because_it_calls_no_model() {
        assert_eq!(Strength::Verbatim.prompt(), None);
    }

    #[test]
    fn every_other_strength_ships_its_instructions() {
        for strength in [Strength::Light, Strength::Balanced, Strength::Heavy] {
            let prompt = strength.prompt().expect("has a prompt");
            assert!(prompt.len() > 200, "{strength:?}: prompt looks truncated");

            // The files are wrapped prose, so a sentence can arrive with a line
            // break in the middle of it.
            let flowed = prompt.split_whitespace().collect::<Vec<_>>().join(" ");

            // The two constraints most likely to break, and the ones whose
            // absence would be worst: a model that answers the dictation, and a
            // model that helpfully translates it.
            assert!(
                flowed.contains("never translate"),
                "{strength:?}: prompt does not forbid translating"
            );
            assert!(
                flowed.contains("You are not an assistant"),
                "{strength:?}: prompt does not forbid answering"
            );
        }
    }

    #[test]
    fn a_normal_cleanup_passes() {
        let said = "so I guess we should um push the review to Thursday no Friday";
        let typed = "So I guess we should push the review to Friday.";
        assert_eq!(guard(said, typed, Strength::Balanced).as_deref(), Ok(typed));
    }

    #[test]
    fn an_answer_instead_of_a_cleanup_is_refused() {
        let said = "should we move the review to Friday or keep it on Thursday what do you think";
        // The failure this stage exists to catch: a model that replied.
        let answered = "Yes, Friday works well.";
        assert!(matches!(
            guard(said, answered, Strength::Balanced),
            Err(Rejection::TooShort { .. })
        ));
    }

    #[test]
    fn an_answer_the_right_length_is_still_refused() {
        // Verbatim from llama3.2:3b, and the reason this guard is not length
        // alone. It is 67% of the question it was asked, which cleared the
        // length floor comfortably, and it is an answer.
        let said = "can you send me the numbers for Q3 when you get a chance";
        let answered = "I don't have any Q3 numbers to send.";

        assert!(
            matches!(
                guard(said, answered, Strength::Balanced),
                Err(Rejection::Invented { .. })
            ),
            "the model answered the dictation and it went through"
        );
    }

    #[test]
    fn a_real_cleanup_keeps_enough_of_the_speakers_words() {
        // Also from the fixture run. "four thirty" became "4:30", which is a
        // cleanup doing its job and costs overlap — the floor has to leave room
        // for it.
        let said = "the deploy is at four thirty and Marcus is on call and the rollback \
                    plan is in the runbook so we should be fine";
        let typed = "The deploy is at 4:30. Marcus is on call. The rollback plan is in the \
                     runbook, so we should be fine.";

        assert!(guard(said, typed, Strength::Balanced).is_ok());
    }

    #[test]
    fn a_summary_instead_of_a_cleanup_is_refused() {
        let said = "we talked about the release and the numbers were fine but the timing is tight \
                    and I think we should push it back by a week to be safe about the testing";
        let summarised = "Push the release back a week.";
        assert!(matches!(
            guard(said, summarised, Strength::Balanced),
            Err(Rejection::TooShort { .. })
        ));
    }

    #[test]
    fn heavy_may_cut_what_balanced_may_not() {
        let said = "so basically what I'm trying to say is that at the end of the day we should \
                    probably just go ahead and push the release back by about a week or so";
        let cut = "We should push the release back a week.";

        assert!(guard(said, cut, Strength::Balanced).is_err());
        assert!(guard(said, cut, Strength::Heavy).is_ok());
    }

    #[test]
    fn a_model_that_wrote_an_essay_is_refused() {
        let said = "move the review to Friday";
        let embellished = "I have moved the review to Friday. Please let me know if this works \
                           for you, and I will send out an updated invitation to everyone.";
        assert!(matches!(
            guard(said, embellished, Strength::Balanced),
            Err(Rejection::TooLong { .. })
        ));
    }

    #[test]
    fn nothing_back_is_refused_rather_than_typed() {
        assert_eq!(
            guard("something", "   ", Strength::Light),
            Err(Rejection::Empty)
        );
    }

    #[test]
    fn the_ratio_does_not_depend_on_the_alphabet() {
        // The same sentence, twice the bytes. A byte ratio would put this
        // outside every bound; a character ratio is 1.0.
        let said = "перенесём ревью на пятницу";
        assert!(guard(said, said, Strength::Balanced).is_ok());
    }
}
