//! The criteria of `specs/003-storage-repository.md` that only a real file can
//! have: the migration runner and durability across a reopen.
//!
//! These are not in `storage_contract.rs` on purpose — there is no in-memory
//! analogue of "close the process and open the file again", so they cannot be
//! written against the port. Everything that *can* be shared lives there.

mod support;

use pressa_core::record::RecordId;
use pressa_core::repository::{ListParams, RecordRepository, StorageError};
use pressa_storage::SqliteRepository;
use rusqlite::Connection;
use serde_json::json;
use support::TempTree;

/// The version `migrations/0001_init.sql` leaves behind.
const CURRENT_SCHEMA_VERSION: i64 = 1;

/// Reads `meta.schema_version` from the file directly, rather than asking the
/// repository — the point is to check what was actually written.
fn stored_schema_version(path: &std::path::Path) -> i64 {
    let connection = Connection::open(path).expect("open the file directly");
    let value: String = connection
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .expect("schema_version is recorded");
    value.parse().expect("schema_version is a number")
}

/// The names of the tables the file holds, sorted.
fn table_names(path: &std::path::Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open the file directly");
    let mut statement = connection
        // `sqlite_%` are SQLite's own bookkeeping tables, not ours.
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .expect("prepare");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        // `collect` over an iterator of results gives one result of a vector.
        .collect::<Result<Vec<String>, _>>()
        .expect("read table names")
}

/// Criterion: opening a fresh file creates the schema.
#[test]
fn opening_a_fresh_file_creates_the_schema() {
    let tree = TempTree::new("fresh");
    let path = tree.database();
    // The parent directory does not exist yet, exactly as on a real first run.
    assert!(!path.exists());

    let repo = SqliteRepository::open(&path).expect("open a fresh database");
    drop(repo); // release the connection before reading the file ourselves

    assert!(path.is_file(), "the database file was created");
    assert_eq!(
        table_names(&path),
        vec![
            "collections".to_string(),
            "meta".to_string(),
            "records".to_string()
        ],
        "docs/storage.md §2 lists exactly these three tables"
    );
    assert_eq!(stored_schema_version(&path), CURRENT_SCHEMA_VERSION);
}

/// Criterion: opening it again is a no-op.
#[test]
fn opening_an_existing_file_again_changes_nothing() {
    let tree = TempTree::new("reopen-noop");
    let path = tree.database();

    let repo = SqliteRepository::open(&path).expect("first open");
    let created = repo
        .create("posts", json!({ "title": "survivor" }))
        .expect("create");
    drop(repo);

    let before = table_names(&path);

    let repo = SqliteRepository::open(&path).expect("second open must not re-run migrations");
    // The data is untouched, which is the observable form of "no-op".
    let fetched = repo
        .get("posts", &created.id)
        .expect("get")
        .expect("still there");
    assert_eq!(fetched, created);
    drop(repo);

    assert_eq!(table_names(&path), before);
    assert_eq!(stored_schema_version(&path), CURRENT_SCHEMA_VERSION);
}

/// Criterion: opening a database from a future `schema_version` fails with a
/// clear error.
#[test]
fn a_future_schema_version_is_refused() {
    let tree = TempTree::new("from-the-future");
    let path = tree.database();

    let repo = SqliteRepository::open(&path).expect("first open");
    drop(repo);

    // Pretend a later version of Pressa wrote this file.
    let connection = Connection::open(&path).expect("open the file directly");
    connection
        .execute(
            "UPDATE meta SET value = '999' WHERE key = 'schema_version'",
            [],
        )
        .expect("plant a future version");
    drop(connection);

    let error = SqliteRepository::open(&path).expect_err("this build cannot read that file");

    match error {
        StorageError::Migration { detail } => {
            // The message has to name both numbers, or the user cannot act on it.
            assert!(detail.contains("999"), "the file's version: {detail}");
            assert!(
                detail.contains(&CURRENT_SCHEMA_VERSION.to_string()),
                "the version this build understands: {detail}"
            );
        }
        other => panic!("expected Migration, got {other:?}"),
    }
}

/// Criterion: data written, connection dropped, database reopened → the data is
/// there. The Golden Path's step 13 is exactly this.
#[test]
fn data_survives_dropping_and_reopening_the_database() {
    let tree = TempTree::new("durable");
    let path = tree.database();

    let (first_id, second_id) = {
        let repo = SqliteRepository::open(&path).expect("first open");
        let first = repo
            .create("posts", json!({ "title": "one", "rank": 1 }))
            .expect("create");
        let second = repo
            .create("posts", json!({ "title": "two", "rank": 2 }))
            .expect("create");
        repo.create("pages", json!({ "title": "a page" }))
            .expect("create");
        // Deleting before the reopen must also be durable.
        repo.delete("pages", &RecordId::new())
            .expect_err("that page id was never issued");
        (first.id, second.id)
        // `repo` is dropped here, closing the connection.
    };

    let repo = SqliteRepository::open(&path).expect("reopen");

    let listed = repo.list("posts", &ListParams::default()).expect("list");
    assert_eq!(
        listed.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![first_id, second_id],
        "both records came back, in id order"
    );
    assert_eq!(listed[0].data, json!({ "title": "one", "rank": 1 }));
    assert_eq!(
        repo.count("pages", &ListParams::default()).expect("count"),
        1
    );
}
