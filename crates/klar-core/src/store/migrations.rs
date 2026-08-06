//! Schema versions, applied in order.
//!
//! SQLite's `user_version` pragma is the whole mechanism: an integer in the
//! file header saying which migrations have run. No migrations table, no
//! checksums, no rollback — this database lives on one machine, is written by
//! one process, and the only thing that can go wrong is a newer Klar than the
//! file expects.
//!
//! **Migrations are append-only.** Editing one that has shipped changes nothing
//! on a machine that already ran it, so the two diverge silently. Add another.

use rusqlite::Connection;

/// Every migration, in order. The index is the version it produces.
const MIGRATIONS: &[&str] = &[
    // v1 — the three tables M5 is about.
    r"
    CREATE TABLE dictionary_entries (
        id           INTEGER PRIMARY KEY,
        -- The spelling the user wants. Unique so teaching a word twice is a
        -- correction rather than a second row that shadows the first.
        term         TEXT    NOT NULL UNIQUE,
        -- What whisper produces instead, one per line.
        replacements TEXT    NOT NULL DEFAULT '',
        enabled      INTEGER NOT NULL DEFAULT 1
    );

    CREATE TABLE dictations (
        id         INTEGER PRIMARY KEY,
        at         INTEGER NOT NULL,
        text       TEXT    NOT NULL,
        raw        TEXT    NOT NULL,
        audio_ms   INTEGER NOT NULL,
        latency_ms INTEGER NOT NULL,
        target     TEXT,
        polished   INTEGER NOT NULL DEFAULT 0
    );

    -- History is read newest first and nothing else.
    CREATE INDEX dictations_at ON dictations (at DESC, id DESC);

    -- Counts per day, kept independently of `dictations` so that clearing the
    -- history does not erase them. See the module docs in `store/mod.rs`.
    CREATE TABLE daily_stats (
        day        TEXT    PRIMARY KEY,
        dictations INTEGER NOT NULL DEFAULT 0,
        words      INTEGER NOT NULL DEFAULT 0,
        chars      INTEGER NOT NULL DEFAULT 0,
        audio_ms   INTEGER NOT NULL DEFAULT 0
    );
    ",
];

/// Bring the database up to the current schema.
pub fn run(conn: &Connection) -> Result<(), rusqlite::Error> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let target = MIGRATIONS.len() as u32;

    if current > target {
        // A file written by a newer Klar. Refusing beats guessing: the columns
        // this build knows may have been renamed under it.
        tracing::warn!(
            found = current,
            understood = target,
            "the database was written by a newer version of Klar"
        );
        return Ok(());
    }

    for (index, migration) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = index as u32 + 1;
        tracing::info!(version, "applying migration");

        // Each migration and its version bump land together, so a crash
        // halfway through cannot leave a schema the header disagrees with.
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(migration)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrating_an_empty_database_creates_the_schema() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();

        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as u32);

        for table in ["dictionary_entries", "dictations", "daily_stats"] {
            let found: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(found, 1, "{table} missing");
        }
    }

    #[test]
    fn migrating_twice_does_nothing_the_second_time() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        run(&conn).unwrap();
    }

    /// A file from a newer Klar is left alone rather than migrated backwards.
    #[test]
    fn a_newer_schema_is_not_touched() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 99u32).unwrap();
        run(&conn).unwrap();

        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 99);
    }
}
