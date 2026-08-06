//! Everything Klar remembers, in one SQLite file on this machine.
//!
//! Three things, and they have different lifetimes on purpose:
//!
//! - **`dictionary_entries`** — the words the user taught Klar. Small, edited
//!   by hand, read on every dictation.
//! - **`dictations`** — what was said and what was inserted. This is the
//!   sensitive one: it is the text of everything dictated, and clearing it is
//!   a button the user is entitled to press.
//! - **`daily_stats`** — counts per day. Deliberately *not* derived from
//!   `dictations`, so that clearing the history does not also erase how much
//!   somebody has used the app. The text is the private part; the totals are
//!   not, and losing a streak is not what "clear my history" means.
//!
//! Nothing here leaves the machine. There is no sync, no telemetry, and no
//! second copy.

mod migrations;

use crate::dictionary::{Dictionary, Entry};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("could not open the database at {path}: {source}")]
    Open {
        path: String,
        #[source]
        source: rusqlite::Error,
    },

    #[error("could not prepare the database: {0}")]
    Migrate(#[source] rusqlite::Error),

    #[error("database error: {0}")]
    Query(#[from] rusqlite::Error),

    #[error("a term cannot be empty")]
    EmptyTerm,
}

/// One finished dictation, on its way to disk.
#[derive(Debug, Clone)]
pub struct NewDictation {
    /// What was inserted, after the dictionary and any polish.
    pub text: String,
    /// What whisper produced, before either. Kept because it is the only way to
    /// tell a recognition problem from a polish problem after the fact — and
    /// it is what a dictionary entry gets written from.
    pub raw: String,
    pub audio_ms: u64,
    /// Key release to inserted text. The number CLAUDE.md's budget is about.
    pub latency_ms: u64,
    /// The application the text went into, when the platform could say.
    pub target: Option<String>,
    pub polished: bool,
}

/// One finished dictation, read back.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dictation {
    pub id: i64,
    /// Unix seconds.
    pub at: i64,
    pub text: String,
    pub raw: String,
    pub audio_ms: u64,
    pub latency_ms: u64,
    pub target: Option<String>,
    pub polished: bool,
}

/// A day's totals.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStat {
    /// `YYYY-MM-DD`, in the user's own timezone — SQLite computes it, so a
    /// dictation at 1 a.m. lands on the day the user thinks it did.
    pub day: String,
    pub dictations: u64,
    pub words: u64,
    pub chars: u64,
    pub audio_ms: u64,
}

/// Everything the stats pane shows, in one query.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub dictations: u64,
    pub words: u64,
    pub chars: u64,
    pub audio_ms: u64,
    /// Days with at least one dictation.
    pub days: u64,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open the database, creating and migrating it as needed.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(path).map_err(|source| StoreError::Open {
            path: path.display().to_string(),
            source,
        })?;

        Self::prepare(conn)
    }

    /// A database that never touches the disk. For tests, and for the `--dry`
    /// paths in `klar-cli` that should not add rows to somebody's real history.
    pub fn in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory().map_err(|source| StoreError::Open {
            path: ":memory:".to_owned(),
            source,
        })?;

        Self::prepare(conn)
    }

    fn prepare(conn: Connection) -> Result<Self, StoreError> {
        // WAL so a read from the settings window cannot block the write at the
        // end of a dictation; foreign keys because SQLite leaves them off.
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(StoreError::Migrate)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(StoreError::Migrate)?;

        let store = Self { conn };
        migrations::run(&store.conn).map_err(StoreError::Migrate)?;
        Ok(store)
    }

    // ---- dictionary ----

    /// Every taught word, enabled or not, oldest first.
    pub fn dictionary(&self) -> Result<Dictionary, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, term, replacements, enabled FROM dictionary_entries ORDER BY id",
        )?;

        let entries = stmt
            .query_map([], |row| {
                let replacements: String = row.get(2)?;
                Ok(Entry {
                    id: Some(row.get(0)?),
                    term: row.get(1)?,
                    // Stored as one line per replacement. Not JSON: these are
                    // words, the column is read by a human debugging a bad
                    // substitution, and a newline is not a thing that appears
                    // in one.
                    replacements: replacements
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    enabled: row.get::<_, i64>(3)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Dictionary::new(entries))
    }

    /// Add a term, or replace the replacements of one already taught.
    ///
    /// Upsert rather than insert: teaching the same word twice is a correction,
    /// not an error, and making the caller check first would be a race.
    pub fn teach(&self, term: &str, replacements: &[String]) -> Result<i64, StoreError> {
        let term = term.trim();
        if term.is_empty() {
            return Err(StoreError::EmptyTerm);
        }

        let joined = replacements
            .iter()
            .map(|replacement| replacement.trim())
            .filter(|replacement| !replacement.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        self.conn.execute(
            "INSERT INTO dictionary_entries (term, replacements, enabled) VALUES (?1, ?2, 1)
             ON CONFLICT(term) DO UPDATE SET replacements = excluded.replacements",
            params![term, joined],
        )?;

        Ok(self.conn.query_row(
            "SELECT id FROM dictionary_entries WHERE term = ?1",
            params![term],
            |row| row.get(0),
        )?)
    }

    pub fn set_term_enabled(&self, id: i64, enabled: bool) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE dictionary_entries SET enabled = ?2 WHERE id = ?1",
            params![id, i64::from(enabled)],
        )?;
        Ok(())
    }

    /// Forget a term. Returns whether there was one.
    pub fn forget(&self, id: i64) -> Result<bool, StoreError> {
        let removed = self
            .conn
            .execute("DELETE FROM dictionary_entries WHERE id = ?1", params![id])?;
        Ok(removed > 0)
    }

    // ---- dictations ----

    /// Record a finished dictation and roll it into the day's totals.
    ///
    /// One transaction: a row in `dictations` without its counts would make the
    /// stats quietly wrong, and there is no later pass that would notice.
    pub fn record(&mut self, dictation: &NewDictation) -> Result<i64, StoreError> {
        let at = now();
        let words = word_count(&dictation.text);
        let chars = dictation.text.chars().count() as u64;

        let tx = self.conn.transaction()?;

        tx.execute(
            "INSERT INTO dictations (at, text, raw, audio_ms, latency_ms, target, polished)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                at,
                dictation.text,
                dictation.raw,
                dictation.audio_ms as i64,
                dictation.latency_ms as i64,
                dictation.target,
                i64::from(dictation.polished),
            ],
        )?;
        let id = tx.last_insert_rowid();

        // `localtime` so the day boundary is the user's midnight, not UTC's.
        tx.execute(
            "INSERT INTO daily_stats (day, dictations, words, chars, audio_ms)
             VALUES (date(?1, 'unixepoch', 'localtime'), 1, ?2, ?3, ?4)
             ON CONFLICT(day) DO UPDATE SET
                 dictations = dictations + 1,
                 words      = words + excluded.words,
                 chars      = chars + excluded.chars,
                 audio_ms   = audio_ms + excluded.audio_ms",
            params![at, words as i64, chars as i64, dictation.audio_ms as i64],
        )?;

        tx.commit()?;
        Ok(id)
    }

    /// The most recent dictations, newest first.
    pub fn history(&self, limit: u32) -> Result<Vec<Dictation>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, at, text, raw, audio_ms, latency_ms, target, polished
             FROM dictations ORDER BY at DESC, id DESC LIMIT ?1",
        )?;

        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(Dictation {
                    id: row.get(0)?,
                    at: row.get(1)?,
                    text: row.get(2)?,
                    raw: row.get(3)?,
                    audio_ms: row.get::<_, i64>(4)? as u64,
                    latency_ms: row.get::<_, i64>(5)? as u64,
                    target: row.get(6)?,
                    polished: row.get::<_, i64>(7)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn forget_dictation(&self, id: i64) -> Result<bool, StoreError> {
        let removed = self
            .conn
            .execute("DELETE FROM dictations WHERE id = ?1", params![id])?;
        Ok(removed > 0)
    }

    /// Delete every dictation. The totals survive — see the module docs.
    ///
    /// `VACUUM` because this is a privacy action: without it the text stays in
    /// the file's free pages, and "cleared" would mean "not shown".
    pub fn clear_history(&self) -> Result<u64, StoreError> {
        let removed = self.conn.execute("DELETE FROM dictations", [])?;
        self.conn.execute_batch("VACUUM")?;
        Ok(removed as u64)
    }

    // ---- stats ----

    /// The last `days` days that have any dictations, newest first.
    pub fn daily(&self, days: u32) -> Result<Vec<DayStat>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT day, dictations, words, chars, audio_ms
             FROM daily_stats ORDER BY day DESC LIMIT ?1",
        )?;

        let rows = stmt
            .query_map(params![days], |row| {
                Ok(DayStat {
                    day: row.get(0)?,
                    dictations: row.get::<_, i64>(1)? as u64,
                    words: row.get::<_, i64>(2)? as u64,
                    chars: row.get::<_, i64>(3)? as u64,
                    audio_ms: row.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn totals(&self) -> Result<Totals, StoreError> {
        let totals = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(dictations), 0), COALESCE(SUM(words), 0),
                        COALESCE(SUM(chars), 0), COALESCE(SUM(audio_ms), 0), COUNT(*)
                 FROM daily_stats",
                [],
                |row| {
                    Ok(Totals {
                        dictations: row.get::<_, i64>(0)? as u64,
                        words: row.get::<_, i64>(1)? as u64,
                        chars: row.get::<_, i64>(2)? as u64,
                        audio_ms: row.get::<_, i64>(3)? as u64,
                        days: row.get::<_, i64>(4)? as u64,
                    })
                },
            )
            .optional()?;

        Ok(totals.unwrap_or(Totals {
            dictations: 0,
            words: 0,
            chars: 0,
            audio_ms: 0,
            days: 0,
        }))
    }

    /// Erase the totals as well. Separate from [`Store::clear_history`] because
    /// they answer different requests: one is "forget what I said", the other
    /// is "forget that I was here".
    pub fn clear_stats(&self) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM daily_stats", [])?;
        Ok(())
    }
}

/// Unix seconds. Saturates at the epoch rather than panicking on a clock set
/// before 1970 — a wrong timestamp on one row is not worth killing a dictation
/// that already succeeded.
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64)
}

/// Words, counted the way a person would: runs of alphanumerics.
fn word_count(text: &str) -> u64 {
    text.split(|ch: char| !ch.is_alphanumeric() && ch != '\'')
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dictation(text: &str) -> NewDictation {
        NewDictation {
            text: text.to_owned(),
            raw: text.to_owned(),
            audio_ms: 2_000,
            latency_ms: 300,
            target: Some("Notepad".to_owned()),
            polished: false,
        }
    }

    #[test]
    fn a_fresh_database_is_empty_rather_than_missing() {
        let store = Store::in_memory().unwrap();
        assert!(store.dictionary().unwrap().entries().is_empty());
        assert!(store.history(10).unwrap().is_empty());
        assert_eq!(store.totals().unwrap().dictations, 0);
    }

    #[test]
    fn a_taught_word_comes_back() {
        let store = Store::in_memory().unwrap();
        store
            .teach(
                "Anthropic",
                &["anthropik".to_owned(), "and thropic".to_owned()],
            )
            .unwrap();

        let dict = store.dictionary().unwrap();
        let entry = &dict.entries()[0];
        assert_eq!(entry.term, "Anthropic");
        assert_eq!(entry.replacements, ["anthropik", "and thropic"]);
        assert!(entry.enabled);
    }

    /// The end-to-end shape of M5's criterion, minus whisper: teach a word,
    /// read the dictionary back, and have it fix the transcript.
    #[test]
    fn a_taught_word_fixes_a_transcript() {
        let store = Store::in_memory().unwrap();
        store.teach("Anthropic", &["anthropik".to_owned()]).unwrap();

        let dict = store.dictionary().unwrap();
        assert_eq!(dict.apply("I work at anthropik"), "I work at Anthropic");
        assert_eq!(dict.prompt().unwrap().0, "Anthropic");
    }

    #[test]
    fn teaching_the_same_term_twice_corrects_it_rather_than_failing() {
        let store = Store::in_memory().unwrap();
        let first = store.teach("Klar", &["clar".to_owned()]).unwrap();
        let second = store
            .teach("Klar", &["clar".to_owned(), "klaar".to_owned()])
            .unwrap();

        assert_eq!(first, second);
        let dict = store.dictionary().unwrap();
        assert_eq!(dict.entries().len(), 1);
        assert_eq!(dict.entries()[0].replacements, ["clar", "klaar"]);
    }

    #[test]
    fn an_empty_term_is_refused() {
        let store = Store::in_memory().unwrap();
        assert!(matches!(
            store.teach("   ", &["x".to_owned()]),
            Err(StoreError::EmptyTerm)
        ));
    }

    #[test]
    fn a_term_can_be_switched_off_and_forgotten() {
        let store = Store::in_memory().unwrap();
        let id = store.teach("Klar", &["clar".to_owned()]).unwrap();

        store.set_term_enabled(id, false).unwrap();
        assert!(!store.dictionary().unwrap().entries()[0].enabled);

        assert!(store.forget(id).unwrap());
        assert!(!store.forget(id).unwrap());
        assert!(store.dictionary().unwrap().entries().is_empty());
    }

    #[test]
    fn a_dictation_is_recorded_and_read_back_newest_first() {
        let mut store = Store::in_memory().unwrap();
        store.record(&dictation("first one")).unwrap();
        store.record(&dictation("second one")).unwrap();

        let history = store.history(10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].text, "second one");
        assert_eq!(history[0].target.as_deref(), Some("Notepad"));
    }

    #[test]
    fn recording_rolls_up_the_day() {
        let mut store = Store::in_memory().unwrap();
        store.record(&dictation("one two three")).unwrap();
        store.record(&dictation("four five")).unwrap();

        let totals = store.totals().unwrap();
        assert_eq!(totals.dictations, 2);
        assert_eq!(totals.words, 5);
        assert_eq!(totals.audio_ms, 4_000);
        assert_eq!(totals.days, 1);

        let daily = store.daily(7).unwrap();
        assert_eq!(daily.len(), 1);
        assert_eq!(daily[0].dictations, 2);
    }

    /// The reason `daily_stats` is a table rather than a query over
    /// `dictations`: clearing the history must not also erase how much somebody
    /// has used the app.
    #[test]
    fn clearing_the_history_keeps_the_totals() {
        let mut store = Store::in_memory().unwrap();
        store.record(&dictation("something private")).unwrap();

        assert_eq!(store.clear_history().unwrap(), 1);
        assert!(store.history(10).unwrap().is_empty());
        assert_eq!(store.totals().unwrap().dictations, 1);

        store.clear_stats().unwrap();
        assert_eq!(store.totals().unwrap().dictations, 0);
    }

    #[test]
    fn a_single_dictation_can_be_forgotten() {
        let mut store = Store::in_memory().unwrap();
        let id = store.record(&dictation("delete me")).unwrap();
        store.record(&dictation("keep me")).unwrap();

        assert!(store.forget_dictation(id).unwrap());
        let history = store.history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].text, "keep me");
    }

    #[test]
    fn words_are_counted_the_way_a_person_would() {
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("   "), 0);
        assert_eq!(word_count("one two three"), 3);
        assert_eq!(word_count("don't count that as two"), 5);
        assert_eq!(word_count("hello, world — again!"), 3);
        assert_eq!(word_count("раз два три"), 3);
    }

    #[test]
    fn a_database_survives_being_closed_and_reopened() {
        let dir = std::env::temp_dir().join(format!("klar-store-{}", std::process::id()));
        let path = dir.join("klar.db");
        let _ = std::fs::remove_dir_all(&dir);

        {
            let mut store = Store::open(&path).unwrap();
            store.teach("Klar", &["clar".to_owned()]).unwrap();
            store.record(&dictation("hello")).unwrap();
        }

        let store = Store::open(&path).unwrap();
        assert_eq!(store.dictionary().unwrap().entries().len(), 1);
        assert_eq!(store.history(10).unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
