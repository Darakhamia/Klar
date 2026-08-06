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
pub mod sidecar;

pub use ollama::{Ollama, OllamaConfig};
pub use sidecar::{Sidecar, SidecarConfig};

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
    ///
    /// The default, because a fresh install has no model server and no model,
    /// and a default that quietly does nothing is worse than one that says it
    /// is off. Turning it on is a deliberate act with a visible setting.
    #[default]
    Verbatim,
    Light,
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

    #[error("the polish model could not be started: {0}")]
    Unstartable(String),

    /// The child process died, or its pipes did. Distinct from
    /// [`PolishError::Unstartable`] because it happens to a polisher that was
    /// working a moment ago, and the answer to it is to start another one.
    #[error("the polish model stopped: {0}")]
    Crashed(String),

    /// The model ran and said it could not do the job. Its own words.
    #[error("the polish model failed: {0}")]
    Model(String),

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
#[derive(Debug, Clone, Default)]
pub struct PolishRequest<'a> {
    pub text: &'a str,
    pub strength: Strength,
    /// Terms the user has taught Klar, so the model does not helpfully correct
    /// a name it has never seen. Empty until M5 fills the dictionary in.
    pub vocabulary: &'a [String],
    /// The ISO code whisper reported for this utterance, when it reported one.
    ///
    /// Not decoration. A 1.5B model handed Russian and a prompt that says
    /// "never translate" translates it anyway about half the time; handed the
    /// same text and a prompt that says "this is Russian, reply in Russian" it
    /// stops. The transcription stage already knows which language it heard,
    /// and this is that knowledge reaching the stage that needs it.
    ///
    /// `None` leaves the instruction out entirely rather than guessing, because
    /// naming the wrong language is worse than naming none.
    pub language: Option<&'a str>,
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
    /// The model Klar ships, in a child process. What a normal install uses.
    Sidecar(Box<Sidecar>),
    /// A model server the user already runs. Not what a normal install uses,
    /// and kept because it is the only way to polish with something bigger than
    /// Klar is willing to download on its own.
    Ollama(Ollama),
}

impl TextPolisher for Polisher {
    async fn polish(&mut self, request: PolishRequest<'_>) -> Result<String, PolishError> {
        match self {
            Self::Noop => Ok(request.text.to_owned()),
            Self::Sidecar(sidecar) => sidecar.polish(request).await,
            Self::Ollama(ollama) => ollama.polish(request).await,
        }
    }

    async fn available(&mut self) -> bool {
        match self {
            Self::Noop => true,
            Self::Sidecar(sidecar) => sidecar.available().await,
            Self::Ollama(ollama) => ollama.available().await,
        }
    }
}

/// The instructions plus whatever the user has taught Klar to spell.
///
/// Shared by every polisher rather than built at each call site: the vocabulary
/// sentence is part of the prompt's behaviour, and two implementations that
/// worded it differently would polish differently for reasons no one would
/// think to look for.
fn system_prompt(instructions: &str, request: &PolishRequest<'_>) -> String {
    let mut prompt = instructions.to_owned();

    // The language, named. Measured on Qwen2.5-1.5B: three Russian dictations
    // through the prompt alone came back translated into English twice; the
    // same three with this line came back in Russian three times out of three.
    // Resolved through whisper's own table so a code it never emits cannot
    // produce a sentence naming a language that is not there.
    if let Some(name) = request.language.and_then(crate::asr::language_name) {
        // whisper's table is lowercase ("russian"); the measurement that
        // justifies this sentence was made with the capitalised name, which is
        // also how a language reads in the middle of an English one.
        let mut name = name.to_owned();
        name[..1].make_ascii_uppercase();
        prompt.push_str(&format!(
            "\n\nThe text you are given is in {name}. Write your reply in {name}."
        ));
    }

    // Names the speaker has taught Klar. Without this a model helpfully
    // "corrects" them, which is the opposite of the dictionary's job.
    if !request.vocabulary.is_empty() {
        prompt.push_str(&format!(
            "\nThese words are spelled correctly and must not be changed: {}.",
            request.vocabulary.join(", ")
        ));
    }

    prompt
}

/// Characters a cleanup may drop before the length floor starts asking
/// questions, however short the dictation was.
///
/// One abandoned clause and the filler introducing it — "в три ну то есть",
/// "to Thursday, no". Forty rather than the twenty-eight that the measured case
/// needed, because the case that produced that number is not the worst one that
/// is still legitimate.
const ALWAYS_DROPPABLE: usize = 40;

/// The floor that still applies once [`ALWAYS_DROPPABLE`] has been forgiven.
///
/// A short dictation may lose most of itself to a correction; it may not lose
/// nearly all of itself. Equal to heavy's own floor, so this can only ever
/// relax a stricter strength down to the most permissive one the guard already
/// trusts, and never below it.
const SHORT_DICTATION_FLOOR: f32 = 0.24;

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

    let after = polished.chars().count();
    let ratio = after as f32 / before as f32;
    let (floor, ceiling) = strength.bounds();

    // The floor asks "is this a summary rather than a cleanup?", and on a long
    // dictation the ratio answers it well. On a short one it does not, because
    // a self-correction is a fixed number of characters and the shorter the
    // sentence the larger a fraction of it that is.
    //
    // Measured, on a real rejection: "давай созвонимся в три ну то есть в
    // четыре я перепутал" polished to "давай созвонимся в четыре." — the
    // correction resolved exactly as the prompt asks, 28 characters dropped out
    // of 54, and refused at 48% against balanced's 55% floor. Correct output,
    // thrown away, and the user would have seen the unpolished transcript with
    // no idea why.
    //
    // So a fixed allowance of dropped characters is forgiven, and below it the
    // floor relaxes to [`SHORT_DICTATION_FLOOR`] rather than disappearing —
    // otherwise a model that answered a short question with "." would pass.
    // Nothing here loosens the long case: dropping 110 characters is still
    // measured against the strength's own floor, whatever the ratio works out
    // to.
    let floor = if before.saturating_sub(after) <= ALWAYS_DROPPABLE {
        floor.min(SHORT_DICTATION_FLOOR)
    } else {
        floor
    };

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
                flowed.contains("same language as the text you were given"),
                "{strength:?}: prompt does not forbid translating"
            );
            assert!(
                flowed.contains("You are not an assistant"),
                "{strength:?}: prompt does not forbid answering"
            );
        }
    }

    /// The rejection that produced [`ALWAYS_DROPPABLE`], kept verbatim.
    ///
    /// Qwen2.5-1.5B did exactly what balanced.md asks — resolved the
    /// correction, dropped the speaker's aside about having mixed it up, stayed
    /// in Russian — and the guard threw it away for being 48% of a 54-character
    /// sentence. A future tightening of the floor should have to walk past this.
    #[test]
    fn a_short_dictation_may_lose_half_of_itself_to_one_correction() {
        let said = "давай созвонимся в три ну то есть в четыре я перепутал";
        let polished = "давай созвонимся в четыре.";

        assert_eq!(
            guard(said, polished, Strength::Balanced).as_deref(),
            Ok(polished),
            "a resolved self-correction is what balanced was asked for"
        );
    }

    /// The other half of that: the allowance must not become a way for a
    /// summary of a long dictation to get through.
    #[test]
    fn a_long_dictation_may_not_lose_the_same_fraction() {
        let said = "so the thing about the migration is that we have about four \
                    hundred tables and the foreign keys are a mess and nobody has \
                    touched the reporting schema since two thousand nineteen";
        let summary = "The migration covers four hundred messy tables.";

        assert!(
            matches!(
                guard(said, summary, Strength::Balanced),
                Err(Rejection::TooShort { .. })
            ),
            "40 characters are forgiven; 130 are a summary"
        );
    }

    /// And the allowance may not let a model answer a short question with
    /// almost nothing.
    #[test]
    fn the_allowance_does_not_permit_an_empty_gesture() {
        let said = "can you send me the numbers for Q3";
        assert!(
            matches!(
                guard(said, ".", Strength::Balanced),
                Err(Rejection::TooShort { .. })
            ),
            "dropping under the short-dictation floor is still a rejection"
        );
    }

    /// The prompt is where the language is named, and it is named because a
    /// 1.5B model does not otherwise keep it. Two Russian dictations out of
    /// three came back in English through the instruction alone; three out of
    /// three stayed Russian once the language was stated.
    #[test]
    fn a_known_language_is_named_to_the_model() {
        let request = PolishRequest {
            text: "перенесём ревью на пятницу",
            strength: Strength::Balanced,
            language: Some("ru"),
            ..PolishRequest::default()
        };
        let prompt = system_prompt("INSTRUCTIONS", &request);

        assert!(
            prompt.contains("in Russian"),
            "the language must be named, not implied: {prompt}"
        );
    }

    /// An unknown or absent code leaves the sentence out rather than writing a
    /// wrong one. Telling a model the text is in "xx" is worse than telling it
    /// nothing.
    #[test]
    fn an_unknown_language_is_left_unsaid() {
        for code in [None, Some("xx"), Some("")] {
            let request = PolishRequest {
                text: "some words",
                strength: Strength::Balanced,
                language: code,
                ..PolishRequest::default()
            };
            let prompt = system_prompt("INSTRUCTIONS", &request);
            assert_eq!(
                prompt, "INSTRUCTIONS",
                "{code:?} should add nothing, got {prompt}"
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

    /// Three real answers from a real model, kept verbatim.
    ///
    /// SmolLM2-360M-Instruct, given `balanced.md` and these transcripts, on the
    /// run that first proved the sidecar worked end to end. Every one of them
    /// is the failure this guard was written for, months before there was a
    /// local model to produce one: the stage was asked to tidy and it answered,
    /// explained, or wrote a commentary on its own edits.
    ///
    /// They are here rather than in a note because they are the only evidence
    /// that the bounds are set where they need to be, and because a later
    /// loosening of them should have to walk past this.
    #[test]
    fn a_small_model_answering_instead_of_tidying_is_refused() {
        let cases = [
            (
                "so I guess we should um push the review to Thursday no Friday and uh I'll write up the notes after",
                "Here's the revised text:\n\n\"I'll review your draft on Thursday, no Friday. I'll write up the notes after that.\"",
            ),
            (
                "can you send me the Q3 numbers when you get a chance",
                "I'm sorry for the misunderstanding, but as an AI text-based model, I don't have access to real-time data or ability to send emails. However, I can certainly help you with your request.",
            ),
            (
                "okay so the plan is like three things first we ship the installer then uh the certificate and then I mean the AI thing",
                "Here's a revised version of the plan with some suggested changes:\n\n\"First, we ship the installer. Then, we provide the certificate. Finally, I'll introduce you to our AI system.\"\n\nI made a few changes to make the plan more concise and clear.",
            ),
        ];

        for (said, answered) in cases {
            let verdict = guard(said, answered, Strength::Balanced);
            assert!(
                verdict.is_err(),
                "a model answering the question got through: {answered:?}"
            );
        }
    }
}
