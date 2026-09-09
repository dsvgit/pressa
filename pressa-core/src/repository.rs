//! The `RecordRepository` **port**: what everything above storage is written
//! against.
//!
//! The port lives here and the adapters live in `pressa-storage`, so the
//! dependency arrow points inward (docs/architecture.md §1). Synchronous by
//! decision — [ADR-0002](../../docs/adr/0002-synchronous-rusqlite.md).
//!
//! The contract the two adapters must both satisfy is
//! `specs/003-storage-repository.md`, enforced by one shared test suite.

use serde_json::Value as Json;

use crate::record::{Record, RecordId};

/// Which way a sort runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

/// How to narrow and order a `list` (docs/storage.md §4).
///
/// `Default` is: no limit, no offset, no search, sort by `id` ascending — which
/// is chronological, because ids are ULIDs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Case-insensitive substring match, applied across `search_fields`.
    pub search: Option<String>,
    /// The fields `search` looks in — normally the collection's `list_columns`.
    ///
    /// Named here rather than derived from the schema because the repository
    /// has no schema: it is handed a collection slug, not a `Collection`.
    pub search_fields: Vec<String>,
    /// Field name to order by. `None` means order by `id`.
    pub sort_by: Option<String>,
    pub sort_direction: SortDirection,
}

impl ListParams {
    /// Sorts by `field` in the given direction, leaving everything else default.
    pub fn sorted_by(field: &str, direction: SortDirection) -> Self {
        ListParams {
            sort_by: Some(field.to_string()),
            sort_direction: direction,
            ..ListParams::default()
        }
    }

    /// Searches for `term` across `fields`, leaving everything else default.
    pub fn searching(term: &str, fields: &[&str]) -> Self {
        ListParams {
            search: Some(term.to_string()),
            // `map` copies each `&str` into an owned `String` the struct can keep.
            search_fields: fields.iter().map(|field| field.to_string()).collect(),
            ..ListParams::default()
        }
    }
}

/// Everything storage can refuse to do.
///
/// `rusqlite::Error` is deliberately *not* a variant: this type is named in the
/// port's signatures, so a rusqlite type here would drag SQLite into
/// `pressa-core` and from there into `pressa-app` and `pressa-tui`
/// (docs/storage.md §6, docs/architecture.md §2). The adapter flattens its
/// backend errors into `Sqlite(String)` instead.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    #[error("no record {id} in collection {collection}")]
    NotFound { collection: String, id: String },

    #[error("unknown collection: {0}")]
    UnknownCollection(String),

    /// Stored `data` is not a JSON object, or an id is not a ULID.
    #[error("record {id} is corrupt: {detail}")]
    Corrupt { id: String, detail: String },

    /// A field name that would be interpolated into SQL is not an identifier.
    ///
    /// Field names reach `json_extract` paths by interpolation, not binding
    /// (docs/storage.md §4), so this check is a security boundary and not a
    /// nicety. Schema loading rejects such names first; this is the backstop.
    #[error("not a usable field name: {field}")]
    InvalidField { field: String },

    /// The database file is at a schema version this build cannot work with.
    #[error("migration failed: {detail}")]
    Migration { detail: String },

    #[error("i/o error: {0}")]
    Io(String),

    #[error("database error: {0}")]
    Sqlite(String),
}

impl StorageError {
    /// The `NotFound` a missing id produces, so both adapters phrase it alike.
    pub fn not_found(collection: &str, id: &RecordId) -> Self {
        StorageError::NotFound {
            collection: collection.to_string(),
            id: id.as_string(),
        }
    }
}

/// The one way anything above storage reaches records.
///
/// Two adapters implement it — `SqliteRepository` and `MemoryRepository` — and
/// they must be indistinguishable through this trait, including error cases and
/// ordering. `&self` rather than `&mut self`: the adapters own their own
/// interior locking, so services can share one repository.
pub trait RecordRepository {
    fn list(&self, collection: &str, params: &ListParams) -> Result<Vec<Record>, StorageError>;

    /// How many records `list` would return with no `limit`/`offset`.
    fn count(&self, collection: &str, params: &ListParams) -> Result<u64, StorageError>;

    fn get(&self, collection: &str, id: &RecordId) -> Result<Option<Record>, StorageError>;

    /// Stores `data` verbatim and stamps both timestamps. Does **not** validate:
    /// that belongs to the service layer, and a repository that validates cannot
    /// be used to repair bad data.
    fn create(&self, collection: &str, data: Json) -> Result<Record, StorageError>;

    /// Replaces `data` wholesale — not a merge — and moves `updated_at`.
    /// `created_at` and `id` are left alone.
    fn update(&self, collection: &str, id: &RecordId, data: Json) -> Result<Record, StorageError>;

    fn delete(&self, collection: &str, id: &RecordId) -> Result<(), StorageError>;

    /// Every record whose `$.<field>` equals `value`. Exists so uniqueness
    /// checks need no query language (domain-model.md §9).
    fn find_by_field(
        &self,
        collection: &str,
        field: &str,
        value: &Json,
    ) -> Result<Vec<Record>, StorageError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_list_is_everything_in_id_order() {
        let params = ListParams::default();
        assert_eq!(params.limit, None);
        assert_eq!(params.offset, None);
        assert_eq!(params.search, None);
        assert_eq!(params.sort_by, None);
        assert_eq!(params.sort_direction, SortDirection::Asc);
    }
}
