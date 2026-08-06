//! The words Klar has been taught.
//!
//! Whisper mangles names it has never seen — a colleague, a product, a piece of
//! jargon — and it mangles them *consistently*, which is what makes this
//! fixable. The user writes down the spelling they want and the shapes it comes
//! out as, and both halves of the pipeline use that:
//!
//! - [`Dictionary::prompt`] goes into whisper's initial prompt, which biases
//!   recognition toward those words before anything is decoded.
//! - [`Dictionary::apply`] runs over the transcript afterwards and fixes what
//!   the bias did not.
//!
//! Two places rather than one because neither is reliable alone. The prompt
//! nudges and cannot be depended on; the substitution is exact and cannot
//! recover a word whisper never produced a recognisable shape for.
//!
//! Nothing here touches the database. The store loads a `Dictionary` and this
//! module decides what it means, so the matching rules can be tested without a
//! file on disk.

/// One taught word.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Row id, or `None` for an entry that has not been saved yet.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    /// The spelling the user wants to see. Substituted in verbatim, because
    /// these are overwhelmingly proper nouns and their capitalisation is the
    /// point.
    pub term: String,

    /// What whisper produces instead. Matched case-insensitively, so
    /// `"anthropic"` covers `"Anthropic"` at the start of a sentence.
    pub replacements: Vec<String>,

    pub enabled: bool,
}

impl Entry {
    /// A new entry, not yet saved.
    pub fn new(term: impl Into<String>, replacements: Vec<String>) -> Self {
        Self {
            id: None,
            term: term.into(),
            replacements,
            enabled: true,
        }
    }
}

/// Every taught word, ready to use.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Dictionary {
    entries: Vec<Entry>,
}

/// Whisper's initial prompt is capped at half its text context — 224 tokens for
/// the large models — and it silently keeps only the tail of anything longer.
/// A dictionary that grew past the limit would quietly stop biasing its oldest
/// terms, so [`Dictionary::prompt`] cuts at a length that stays inside it and
/// says how many terms it dropped rather than losing them invisibly.
///
/// Characters rather than tokens: counting tokens needs whisper's tokeniser,
/// which lives behind a loaded model, and four characters per token is the
/// conservative end of what these words cost.
const PROMPT_LIMIT: usize = 800;

impl Dictionary {
    pub fn new(entries: Vec<Entry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|entry| !entry.enabled)
    }

    /// The terms, as a sentence for whisper's `initial_prompt`.
    ///
    /// `None` when there is nothing enabled — an empty prompt is not the same
    /// as no prompt, and passing one costs a little accuracy for nothing.
    ///
    /// Returns the terms that fit and the number left out.
    pub fn prompt(&self) -> Option<(String, usize)> {
        let terms: Vec<&str> = self
            .entries
            .iter()
            .filter(|entry| entry.enabled && !entry.term.trim().is_empty())
            .map(|entry| entry.term.as_str())
            .collect();

        if terms.is_empty() {
            return None;
        }

        let mut prompt = String::new();
        let mut dropped = 0;
        for term in terms {
            // +2 for the separator this term would need.
            if !prompt.is_empty() && prompt.len() + term.len() + 2 > PROMPT_LIMIT {
                dropped += 1;
                continue;
            }
            if !prompt.is_empty() {
                prompt.push_str(", ");
            }
            prompt.push_str(term);
        }

        Some((prompt, dropped))
    }

    /// Replace every taught mis-hearing with the spelling the user wants.
    ///
    /// Case-insensitive, whitespace-flexible, and bounded to whole words: a
    /// term for `"Ana"` must not turn `"analysis"` into `"Anasis"`. Longer
    /// phrases win over shorter ones, so an entry for `"Claude Code"` is not
    /// half-eaten by one for `"Claude"`.
    pub fn apply(&self, text: &str) -> String {
        let phrases = self.phrases();
        if phrases.is_empty() {
            return text.to_owned();
        }

        let mut out = String::with_capacity(text.len());
        let mut at = 0;

        while at < text.len() {
            if !text.is_char_boundary(at) {
                at += 1;
                continue;
            }

            let replaced = starts_word(text, at)
                .then(|| {
                    phrases.iter().find_map(|(phrase, term)| {
                        let taken = match_at(text, at, phrase)?;
                        ends_word(text, at + taken).then_some((taken, term))
                    })
                })
                .flatten();

            match replaced {
                Some((taken, term)) => {
                    out.push_str(term);
                    at += taken;
                }
                None => {
                    let ch = text[at..].chars().next().unwrap_or('\0');
                    out.push(ch);
                    at += ch.len_utf8();
                }
            }
        }

        out
    }

    /// Every enabled replacement paired with what it becomes, longest first.
    ///
    /// Longest first is what makes the match greedy: the scan takes the first
    /// phrase that fits at a position, so the order here *is* the precedence
    /// rule.
    fn phrases(&self) -> Vec<(&str, &str)> {
        let mut phrases: Vec<(&str, &str)> = self
            .entries
            .iter()
            .filter(|entry| entry.enabled)
            .flat_map(|entry| {
                entry
                    .replacements
                    .iter()
                    .map(|replacement| (replacement.trim(), entry.term.as_str()))
            })
            .filter(|(replacement, _)| !replacement.is_empty())
            .collect();

        phrases.sort_by_key(|(replacement, _)| std::cmp::Reverse(replacement.len()));
        phrases
    }
}

/// Whether a replacement may begin at this byte — that is, whether the
/// character before it ends a word.
fn starts_word(text: &str, at: usize) -> bool {
    text[..at]
        .chars()
        .next_back()
        .is_none_or(|ch| !ch.is_alphanumeric())
}

/// Whether a match ending at this byte stops at a word boundary.
fn ends_word(text: &str, at: usize) -> bool {
    text[at..]
        .chars()
        .next()
        .is_none_or(|ch| !ch.is_alphanumeric())
}

/// Match `phrase` against `text` at `at`, ignoring case, and treating any run
/// of whitespace in the phrase as any run of whitespace in the text.
///
/// Returns how many bytes of `text` the match consumed, which is not
/// necessarily the phrase's own length — `"claude  code"` in the transcript is
/// two bytes longer than the phrase `"claude code"`, and the caller has to know
/// how far to skip.
///
/// Case folding is per character rather than by lowercasing both sides, because
/// lowercasing changes byte lengths for some characters and the caller needs
/// offsets into the original.
fn match_at(text: &str, at: usize, phrase: &str) -> Option<usize> {
    let mut haystack = text[at..].chars().peekable();
    let mut needle = phrase.chars().peekable();
    let mut taken = 0;

    while let Some(&wanted) = needle.peek() {
        if wanted.is_whitespace() {
            // One run of whitespace matches another, of any length.
            while needle.peek().is_some_and(|ch| ch.is_whitespace()) {
                needle.next();
            }
            let mut ran = false;
            while haystack.peek().is_some_and(|ch| ch.is_whitespace()) {
                taken += haystack.next()?.len_utf8();
                ran = true;
            }
            if !ran {
                return None;
            }
            continue;
        }

        let found = *haystack.peek()?;
        if !found.to_lowercase().eq(wanted.to_lowercase()) {
            return None;
        }
        taken += found.len_utf8();
        haystack.next();
        needle.next();
    }

    Some(taken)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dictionary(entries: &[(&str, &[&str])]) -> Dictionary {
        Dictionary::new(
            entries
                .iter()
                .map(|(term, replacements)| {
                    Entry::new(
                        *term,
                        replacements.iter().map(|r| (*r).to_owned()).collect(),
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn an_empty_dictionary_changes_nothing() {
        let text = "so I said we should ship it";
        assert_eq!(Dictionary::default().apply(text), text);
    }

    #[test]
    fn a_mis_hearing_becomes_the_taught_spelling() {
        let dict = dictionary(&[("Anthropic", &["anthropik", "and thropic"])]);
        assert_eq!(dict.apply("I work at anthropik"), "I work at Anthropic");
        assert_eq!(dict.apply("I work at and thropic"), "I work at Anthropic");
    }

    /// The case that makes a naive `str::replace` wrong: the same letters
    /// inside a longer word are a different word.
    #[test]
    fn a_term_does_not_eat_the_middle_of_a_longer_word() {
        let dict = dictionary(&[("Ana", &["ana"])]);
        assert_eq!(
            dict.apply("the analysis of banana"),
            "the analysis of banana"
        );
        assert_eq!(dict.apply("ana said so"), "Ana said so");
    }

    #[test]
    fn matching_ignores_case_and_the_replacement_keeps_its_own() {
        let dict = dictionary(&[("PostgreSQL", &["postgres sequel"])]);
        assert_eq!(dict.apply("Postgres Sequel is fine"), "PostgreSQL is fine");
    }

    #[test]
    fn a_longer_phrase_wins_over_a_shorter_one() {
        let dict = dictionary(&[("Claude Code", &["cloud code"]), ("Claude", &["cloud"])]);
        assert_eq!(
            dict.apply("I used cloud code today"),
            "I used Claude Code today"
        );
        assert_eq!(dict.apply("I used cloud today"), "I used Claude today");
    }

    #[test]
    fn extra_whitespace_between_words_still_matches() {
        let dict = dictionary(&[("Claude Code", &["cloud code"])]);
        assert_eq!(dict.apply("use cloud  code"), "use Claude Code");
    }

    #[test]
    fn punctuation_around_a_match_survives() {
        let dict = dictionary(&[("Anthropic", &["anthropik"])]);
        assert_eq!(
            dict.apply("(anthropik), yes — anthropik."),
            "(Anthropic), yes — Anthropic."
        );
    }

    /// Klar is used in Russian as much as English, and `char::is_alphanumeric`
    /// is what makes the boundary rule hold outside ASCII.
    #[test]
    fn non_ascii_words_match_and_keep_their_boundaries() {
        let dict = dictionary(&[("Клар", &["клара", "клар"])]);
        assert_eq!(dict.apply("привет, клар!"), "привет, Клар!");
        assert_eq!(dict.apply("кларнет играет"), "кларнет играет");
    }

    #[test]
    fn a_disabled_entry_does_nothing() {
        let mut dict = dictionary(&[("Anthropic", &["anthropik"])]);
        dict.entries[0].enabled = false;
        assert_eq!(dict.apply("at anthropik"), "at anthropik");
        assert!(dict.prompt().is_none());
        assert!(dict.is_empty());
    }

    #[test]
    fn the_prompt_lists_the_terms() {
        let dict = dictionary(&[("Anthropic", &["anthropik"]), ("Klar", &["clar"])]);
        let (prompt, dropped) = dict.prompt().unwrap();
        assert_eq!(prompt, "Anthropic, Klar");
        assert_eq!(dropped, 0);
    }

    #[test]
    fn no_terms_means_no_prompt_rather_than_an_empty_one() {
        assert!(Dictionary::default().prompt().is_none());
        assert!(dictionary(&[("   ", &["x"])]).prompt().is_none());
    }

    /// Whisper keeps only the tail of an over-long prompt, so a dictionary that
    /// outgrows the limit must drop terms visibly rather than lose the oldest
    /// ones without saying so.
    #[test]
    fn an_over_long_prompt_is_cut_and_reports_what_it_dropped() {
        let entries: Vec<Entry> = (0..200)
            .map(|n| Entry::new(format!("Term{n:04}"), vec![format!("term {n}")]))
            .collect();
        let (prompt, dropped) = Dictionary::new(entries).prompt().unwrap();

        assert!(prompt.len() <= PROMPT_LIMIT, "{} chars", prompt.len());
        assert!(dropped > 0);
        assert_eq!(prompt.split(", ").count() + dropped, 200);
    }

    #[test]
    fn a_replacement_that_is_only_whitespace_is_ignored() {
        let dict = dictionary(&[("Anthropic", &["   ", ""])]);
        assert_eq!(dict.apply("nothing to do here"), "nothing to do here");
    }
}
