//! `SqliteRepository` — the `RecordRepository` adapter over one SQLite file.
//!
//! Synchronous `rusqlite` with the `bundled` feature
//! ([ADR-0002](../../docs/adr/0002-synchronous-rusqlite.md)): a single-user
//! local TUI has no concurrency to manage, so one `Connection` behind a `Mutex`
//! is the whole story and a pool would be complexity without a problem.
//!
//! Records are JSON documents in one table
//! ([ADR-0003](../../docs/adr/0003-json-documents-not-eav.md)); the SQL mapping
//! is docs/storage.md §4.

use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, Row, params_from_iter};
use serde_json::Value as Json;

use pressa_core::record::{Record, RecordId};
use pressa_core::repository::{ListParams, RecordRepository, SortDirection, StorageError};

use crate::clock;
use crate::migrations;
use crate::values::{JsonKey, LIKE_ESCAPE, checked_field, like_pattern, require_object};

/// The columns a `Record` is built from, in the order [`raw_row`] reads them.
const COLUMNS: &str = "id, collection, data, created_at, updated_at";

/// Flattens a backend error into a string.
///
/// `rusqlite::Error` is deliberately not carried through: `StorageError` is
/// named in the port's signatures, so a rusqlite type in it would drag SQLite
/// into `pressa-core` and from there into the whole workspace
/// (docs/storage.md §6).
pub(crate) fn from_sqlite(error: rusqlite::Error) -> StorageError {
    StorageError::Sqlite(error.to_string())
}

#[derive(Debug)]
pub struct SqliteRepository {
    connection: Mutex<Connection>,
}

impl SqliteRepository {
    /// Opens (or creates) the database at `path`, sets the pragmas and runs the
    /// migrations. Parent directories are created: on a first run `.pressa/` is
    /// not there yet.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        // `parent` is `None` only for a root path, and empty for a bare filename.
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|error| StorageError::Io(format!("{}: {error}", parent.display())))?;
            }
        }

        // `mut` because the migration runner needs a transaction, which borrows
        // the connection mutably.
        let mut connection = Connection::open(path).map_err(from_sqlite)?;
        configure(&connection)?;
        migrations::apply(&mut connection)?;

        Ok(SqliteRepository {
            connection: Mutex::new(connection),
        })
    }

    /// The connection, locked for the duration of one operation.
    ///
    /// A poisoned lock means another thread panicked while holding it. Every
    /// operation here is a single statement or its own transaction, so the
    /// connection is still consistent and recovering beats refusing to work.
    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The pragmas of docs/storage.md §5, applied on every open.
fn configure(connection: &Connection) -> Result<(), StorageError> {
    // `journal_mode` answers with the mode it settled on, so it has to be
    // queried rather than executed — `execute` refuses a statement that returns.
    let _mode: String = connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(from_sqlite)?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(from_sqlite)?;
    connection
        .busy_timeout(Duration::from_millis(5000))
        .map_err(from_sqlite)?;
    Ok(())
}

/// One row, still as the strings SQLite stores.
///
/// Read as text and converted afterwards, because a bad ULID or unparseable
/// JSON is a `StorageError::Corrupt` and the row-mapping closure can only
/// return a `rusqlite::Error`.
struct RawRow {
    id: String,
    collection: String,
    data: String,
    created_at: String,
    updated_at: String,
}

fn raw_row(row: &Row<'_>) -> rusqlite::Result<RawRow> {
    Ok(RawRow {
        id: row.get(0)?,
        collection: row.get(1)?,
        data: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

/// Turns a stored row into a `Record`, or reports the row as corrupt.
fn record_from_raw(raw: RawRow) -> Result<Record, StorageError> {
    // `parse` picks `RecordId`'s `FromStr`; a stored id that is not a ULID means
    // something other than pressa wrote this row.
    let id: RecordId = raw.id.parse().map_err(|_| StorageError::Corrupt {
        id: raw.id.clone(),
        detail: "id is not a ULID".to_string(),
    })?;

    let data: Json = serde_json::from_str(&raw.data).map_err(|error| StorageError::Corrupt {
        id: raw.id.clone(),
        detail: format!("data is not valid JSON: {error}"),
    })?;
    require_object(&id, &data)?;

    Ok(Record {
        id,
        collection: raw.collection,
        data,
        created_at: clock::parse(&id, &raw.created_at)?,
        updated_at: clock::parse(&id, &raw.updated_at)?,
    })
}

/// A `WHERE` clause and the values it binds.
struct Filter {
    clause: String,
    values: Vec<SqlValue>,
}

/// Builds `WHERE collection = ? [AND (search…)]`.
///
/// Search *values* are bound; the *field list* is interpolated, which is why
/// every name goes through [`checked_field`] first (docs/storage.md §4).
fn build_filter(collection: &str, params: &ListParams) -> Result<Filter, StorageError> {
    let mut clause = String::from(" WHERE collection = ?1");
    let mut values = vec![SqlValue::Text(collection.to_string())];

    if let Some(term) = &params.search {
        let mut disjuncts = Vec::new();
        for field in &params.search_fields {
            let field = checked_field(field)?;
            values.push(SqlValue::Text(like_pattern(term)));
            disjuncts.push(format!(
                "lower(json_extract(data, '$.{field}')) LIKE ?{} ESCAPE '{LIKE_ESCAPE}'",
                values.len() // 1-based, and this value was just pushed
            ));
        }

        if disjuncts.is_empty() {
            // Searching with no field named can match nothing, and `1 = 0` is how
            // SQL says so without a special case downstream.
            clause.push_str(" AND 1 = 0");
        } else {
            clause.push_str(&format!(" AND ({})", disjuncts.join(" OR ")));
        }
    }

    Ok(Filter { clause, values })
}

/// Builds the `ORDER BY`.
///
/// A field sort appends `id ASC` so the order is total: without it, rows with
/// equal keys could come back in a different order between two renders and the
/// snapshot tests would be flaky (docs/storage.md §4).
fn build_order_by(params: &ListParams) -> Result<String, StorageError> {
    let direction = match params.sort_direction {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };

    match &params.sort_by {
        // Ordering by id is chronological, because ids are ULIDs.
        None => Ok(format!(" ORDER BY id {direction}")),
        Some(field) => {
            let field = checked_field(field)?;
            Ok(format!(
                " ORDER BY json_extract(data, '$.{field}') {direction}, id ASC"
            ))
        }
    }
}

/// Appends `LIMIT`/`OFFSET` if either is asked for, binding both.
fn push_limit_offset(sql: &mut String, values: &mut Vec<SqlValue>, params: &ListParams) {
    if params.limit.is_none() && params.offset.is_none() {
        return;
    }

    // SQLite ignores OFFSET unless LIMIT is present, and -1 means "no limit" —
    // so an offset without a limit still works.
    let limit = params.limit.map(i64::from).unwrap_or(-1);
    values.push(SqlValue::Integer(limit));
    sql.push_str(&format!(" LIMIT ?{}", values.len()));

    values.push(SqlValue::Integer(params.offset.map(i64::from).unwrap_or(0)));
    sql.push_str(&format!(" OFFSET ?{}", values.len()));
}

/// The value to bind when comparing against `json_extract`.
///
/// Each kind binds as the storage class SQLite would have kept it in, so an
/// integer past 2^53 compares by its digits rather than by the nearest double.
/// Across the two kinds SQLite still compares numerically, so a stored `7`
/// matches a searched `7.0` — which is what `JsonKey` promises.
fn bound_value(key: &JsonKey) -> SqlValue {
    match key {
        JsonKey::Null => SqlValue::Null,
        JsonKey::Integer(whole) => SqlValue::Integer(*whole),
        JsonKey::Real(number) => SqlValue::Real(*number),
        JsonKey::Text(text) => SqlValue::Text(text.clone()),
    }
}

impl SqliteRepository {
    /// Runs a `SELECT` of [`COLUMNS`] and materialises every row.
    fn query_records(&self, sql: &str, values: &[SqlValue]) -> Result<Vec<Record>, StorageError> {
        let connection = self.connection();
        let mut statement = connection.prepare(sql).map_err(from_sqlite)?;
        // `params_from_iter` binds a slice whose length is only known at runtime.
        let rows = statement
            .query_map(params_from_iter(values.iter()), raw_row)
            .map_err(from_sqlite)?;

        let mut records = Vec::new();
        for row in rows {
            // Two failures to unwrap here: reading the row, then trusting it.
            records.push(record_from_raw(row.map_err(from_sqlite)?)?);
        }
        Ok(records)
    }
}

impl RecordRepository for SqliteRepository {
    fn list(&self, collection: &str, params: &ListParams) -> Result<Vec<Record>, StorageError> {
        let filter = build_filter(collection, params)?;
        let mut sql = format!("SELECT {COLUMNS} FROM records{}", filter.clause);
        sql.push_str(&build_order_by(params)?);

        let mut values = filter.values;
        push_limit_offset(&mut sql, &mut values, params);

        self.query_records(&sql, &values)
    }

    fn count(&self, collection: &str, params: &ListParams) -> Result<u64, StorageError> {
        let filter = build_filter(collection, params)?;
        // Counting needs no ordering, but a bad sort field is still refused, so
        // that `count` and `list` fail the same way on the same params.
        if let Some(field) = &params.sort_by {
            checked_field(field)?;
        }

        let sql = format!("SELECT count(*) FROM records{}", filter.clause);
        let connection = self.connection();
        let total: i64 = connection
            .query_row(&sql, params_from_iter(filter.values.iter()), |row| {
                row.get(0)
            })
            .map_err(from_sqlite)?;

        // `count(*)` is never negative; the clamp is there so the cast is honest.
        Ok(total.max(0) as u64)
    }

    fn get(&self, collection: &str, id: &RecordId) -> Result<Option<Record>, StorageError> {
        let sql = format!("SELECT {COLUMNS} FROM records WHERE collection = ?1 AND id = ?2");
        let connection = self.connection();
        let raw = connection
            .query_row(&sql, (collection, id.as_string()), raw_row)
            .optional() // "no such row" is `Ok(None)`, not a failure
            .map_err(from_sqlite)?;

        // `transpose` turns an optional fallible conversion inside out.
        raw.map(record_from_raw).transpose()
    }

    fn create(&self, collection: &str, data: Json) -> Result<Record, StorageError> {
        let id = RecordId::new();
        let at = clock::now();
        let stamp = clock::format(&at);

        let connection = self.connection();
        connection
            .execute(
                // ?4 twice: created_at and updated_at start out identical.
                "INSERT INTO records (id, collection, data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                (id.as_string(), collection, data.to_string(), &stamp),
            )
            .map_err(from_sqlite)?;

        Ok(Record {
            id,
            collection: collection.to_string(),
            // Stored verbatim, unvalidated: a repository that validates cannot
            // be used to repair bad data.
            data,
            created_at: at,
            updated_at: at,
        })
    }

    fn update(&self, collection: &str, id: &RecordId, data: Json) -> Result<Record, StorageError> {
        let at = clock::now();
        let stamp = clock::format(&at);

        let connection = self.connection();
        // `RETURNING created_at` does the "does it exist" check and fetches the
        // one column we must not overwrite, in a single statement.
        let created_at: Option<String> = connection
            .query_row(
                "UPDATE records SET data = ?3, updated_at = ?4
                 WHERE collection = ?1 AND id = ?2
                 RETURNING created_at",
                (collection, id.as_string(), data.to_string(), &stamp),
                |row| row.get(0),
            )
            .optional()
            .map_err(from_sqlite)?;

        // No row came back, so nothing was updated — never a silent success.
        let Some(created_at) = created_at else {
            return Err(StorageError::not_found(collection, id));
        };

        Ok(Record {
            id: *id,
            collection: collection.to_string(),
            // Replaced wholesale, not merged.
            data,
            created_at: clock::parse(id, &created_at)?,
            updated_at: at,
        })
    }

    fn delete(&self, collection: &str, id: &RecordId) -> Result<(), StorageError> {
        let connection = self.connection();
        let affected = connection
            .execute(
                "DELETE FROM records WHERE collection = ?1 AND id = ?2",
                (collection, id.as_string()),
            )
            .map_err(from_sqlite)?;

        if affected == 0 {
            Err(StorageError::not_found(collection, id))
        } else {
            Ok(())
        }
    }

    fn find_by_field(
        &self,
        collection: &str,
        field: &str,
        value: &Json,
    ) -> Result<Vec<Record>, StorageError> {
        let field = checked_field(field)?;
        let bound = bound_value(&JsonKey::of(Some(value)));

        // `IS` rather than `=`: it is null-safe, so searching for a null value
        // finds the rows where the field is absent instead of finding nothing.
        let sql = format!(
            "SELECT {COLUMNS} FROM records
             WHERE collection = ?1 AND json_extract(data, '$.{field}') IS ?2
             ORDER BY id ASC"
        );

        self.query_records(&sql, &[SqlValue::Text(collection.to_string()), bound])
    }
}
