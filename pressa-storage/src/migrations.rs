//! The migration runner (docs/storage.md §3).
//!
//! Hand-rolled and about forty lines: `refinery` would be a dependency and an
//! ADR for less. Forward-only — there are no down migrations, because this is a
//! local file the user can delete.

use rusqlite::{Connection, OptionalExtension};

use pressa_core::repository::StorageError;

use crate::sqlite::from_sqlite;

/// Where `meta` records the highest applied migration.
const VERSION_KEY: &str = "schema_version";

/// Every migration, embedded so the binary carries its own schema.
const MIGRATIONS: [(u32, &str); 1] = [(1, include_str!("../migrations/0001_init.sql"))];

/// The version a freshly migrated database ends up at.
pub fn latest_version() -> u32 {
    // `max` is `None` only for an empty list, which would mean no schema at all.
    MIGRATIONS
        .iter()
        .map(|(version, _)| *version)
        .max()
        .unwrap_or(0)
}

/// Brings `connection` up to [`latest_version`], or explains why it cannot.
///
/// Everything happens in one transaction: a half-applied schema is worse than
/// no schema, because the next open would see a version it cannot trust.
pub fn apply(connection: &mut Connection) -> Result<(), StorageError> {
    let current = stored_version(connection)?;
    let latest = latest_version();

    if current > latest {
        // Refused rather than guessed at: a newer Pressa may have changed the
        // meaning of columns this build thinks it understands.
        return Err(StorageError::Migration {
            detail: format!(
                "database is at schema version {current}, but this build of pressa \
                 understands only up to {latest}; use a newer pressa"
            ),
        });
    }

    if current == latest {
        return Ok(()); // already current: opening an existing file is a no-op
    }

    // `transaction` borrows the connection mutably until it is committed.
    let transaction = connection.transaction().map_err(from_sqlite)?;
    for (version, script) in MIGRATIONS {
        if version > current {
            // `execute_batch` runs several statements from one string.
            transaction.execute_batch(script).map_err(from_sqlite)?;
        }
    }
    transaction
        .execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (VERSION_KEY, latest.to_string()),
        )
        .map_err(from_sqlite)?;
    transaction.commit().map_err(from_sqlite)?;

    Ok(())
}

/// The version recorded in the file: `0` for a database that has none yet.
fn stored_version(connection: &Connection) -> Result<u32, StorageError> {
    // `meta` is itself created by the first migration, so on a fresh file the
    // table is not there and asking for its contents would be an error.
    let meta_exists: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'meta'",
            [],
            |row| row.get(0),
        )
        .map_err(from_sqlite)?;
    if meta_exists == 0 {
        return Ok(0);
    }

    // `optional` turns "no such row" into `None` instead of an error.
    let recorded: Option<String> = connection
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            (VERSION_KEY,),
            |row| row.get(0),
        )
        .optional()
        .map_err(from_sqlite)?;

    match recorded {
        None => Ok(0),
        // A version that is not a number is not something to round down to zero:
        // that would re-run migration 1 over a populated database.
        Some(text) => text.parse().map_err(|_| StorageError::Migration {
            detail: format!("{VERSION_KEY} is {text:?}, which is not a version number"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_connection_reports_no_schema_yet() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        assert_eq!(stored_version(&connection).expect("version"), 0);
    }

    #[test]
    fn applying_twice_leaves_the_same_version() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        apply(&mut connection).expect("first apply");
        assert_eq!(
            stored_version(&connection).expect("version"),
            latest_version()
        );

        apply(&mut connection).expect("second apply is a no-op");
        assert_eq!(
            stored_version(&connection).expect("version"),
            latest_version()
        );
    }

    #[test]
    fn a_version_that_is_not_a_number_is_refused() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        apply(&mut connection).expect("apply");
        connection
            .execute(
                "UPDATE meta SET value = 'banana' WHERE key = 'schema_version'",
                [],
            )
            .expect("plant nonsense");

        let error = apply(&mut connection).expect_err("nonsense is not a version");
        assert!(matches!(error, StorageError::Migration { .. }));
    }
}
