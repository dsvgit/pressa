//! `MemoryRepository` — the same port, backed by a map instead of a file.
//!
//! It exists so `pressa-app` and `pressa-tui` tests never open a database, and
//! so a failing service test cannot be blamed on SQL (docs/storage.md §5).
//!
//! That only works if it is **indistinguishable** from `SqliteRepository`
//! through the trait, so this file deliberately imitates SQL rather than doing
//! the obvious Rust thing: values are compared as `crate::values::JsonKey`,
//! matching is ASCII-case-insensitive like SQLite's `lower()`, and rows are
//! checked for corruption at the same point in the pipeline the SQL would
//! materialise them. `tests/storage_contract.rs` is what holds the two together.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use serde_json::Value as Json;

use pressa_core::record::{Record, RecordId};
use pressa_core::repository::{ListParams, RecordRepository, SortDirection, StorageError};

use crate::clock;
use crate::values::{JsonKey, checked_field, contains_ignoring_ascii_case, require_object};

/// A `BTreeMap` rather than the `IndexMap` of docs/storage.md §5: keys are
/// ULIDs, so tree order *is* id order, which is the default list order. That
/// holds even for records inserted out of sequence, which insertion order would
/// not — and it needs no extra dependency.
#[derive(Debug, Default)]
pub struct MemoryRepository {
    records: Mutex<BTreeMap<RecordId, Record>>,
}

impl MemoryRepository {
    pub fn new() -> Self {
        MemoryRepository::default()
    }

    /// The map, locked for one operation. A poisoned lock is recovered from for
    /// the same reason as in `SqliteRepository`: every operation is atomic on
    /// its own, so the map is never left half-written.
    fn records(&self) -> MutexGuard<'_, BTreeMap<RecordId, Record>> {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Whether a record satisfies `params.search`.
///
/// No search means every record qualifies. A search with no field named can
/// match nothing, which is the `1 = 0` the SQL builder emits.
fn matches_search(record: &Record, params: &ListParams) -> bool {
    // `let ... else` returns early when there is nothing to match against.
    let Some(term) = &params.search else {
        return true;
    };

    // `any` short-circuits on the first field that matches.
    params.search_fields.iter().any(|field| {
        JsonKey::at(&record.data, field)
            .as_search_text()
            // `is_some_and` skips a null field: no `LIKE` matches NULL.
            .is_some_and(|text| contains_ignoring_ascii_case(&text, term))
    })
}

/// Orders `records` the way the `ORDER BY` of docs/storage.md §4 would.
fn sort(records: &mut [Record], params: &ListParams) {
    match &params.sort_by {
        None => {
            // The map already handed them over in id order, so ascending is done.
            if params.sort_direction == SortDirection::Desc {
                records.reverse();
            }
        }
        Some(field) => {
            records.sort_by(|left, right| {
                let ordering = JsonKey::at(&left.data, field).cmp(&JsonKey::at(&right.data, field));
                match params.sort_direction {
                    SortDirection::Asc => ordering,
                    SortDirection::Desc => ordering.reverse(),
                }
            });
            // No explicit tiebreaker is needed: `sort_by` is stable and the input
            // was in id order, so equal keys stay in id order — which is exactly
            // what the trailing `id ASC` does in SQL, in both directions.
        }
    }
}

/// Applies `limit` and `offset`, as the SQL clause of the same name would.
fn paginate(records: Vec<Record>, params: &ListParams) -> Vec<Record> {
    // `unwrap_or(0)` because no offset means start at the beginning.
    let offset = params.offset.unwrap_or(0) as usize;
    match params.limit {
        Some(limit) => records
            .into_iter()
            .skip(offset)
            .take(limit as usize)
            .collect(),
        None => records.into_iter().skip(offset).collect(),
    }
}

impl MemoryRepository {
    /// Every record of `collection` that `params.search` accepts, in id order.
    ///
    /// Corruption is **not** checked here: SQLite only materialises the rows it
    /// selected, so a corrupt row that the search filtered out or that paging
    /// skipped must not fail either adapter.
    fn selected(&self, collection: &str, params: &ListParams) -> Vec<Record> {
        self.records()
            .values()
            .filter(|record| record.collection == collection)
            .filter(|record| matches_search(record, params))
            // `cloned` copies each record out so the lock can be released.
            .cloned()
            .collect()
    }

    /// Refuses any field name that the SQL builder would interpolate.
    fn check_field_names(params: &ListParams) -> Result<(), StorageError> {
        if let Some(field) = &params.sort_by {
            checked_field(field)?;
        }
        // Only when there is a search: with none, the SQL never names them either.
        if params.search.is_some() {
            for field in &params.search_fields {
                checked_field(field)?;
            }
        }
        Ok(())
    }
}

impl RecordRepository for MemoryRepository {
    fn list(&self, collection: &str, params: &ListParams) -> Result<Vec<Record>, StorageError> {
        MemoryRepository::check_field_names(params)?;

        let mut selected = self.selected(collection, params);
        sort(&mut selected, params);
        let page = paginate(selected, params);

        // Only now, on the rows that survived: this is where SQL would build them.
        for record in &page {
            require_object(&record.id, &record.data)?;
        }
        Ok(page)
    }

    fn count(&self, collection: &str, params: &ListParams) -> Result<u64, StorageError> {
        MemoryRepository::check_field_names(params)?;
        // Paging does not change how many there are, and `count(*)` never reads
        // `data`, so a corrupt row is counted rather than refused.
        Ok(self.selected(collection, params).len() as u64)
    }

    fn get(&self, collection: &str, id: &RecordId) -> Result<Option<Record>, StorageError> {
        let records = self.records();
        // A record of another collection is not reachable under this slug.
        let found = records
            .get(id)
            .filter(|record| record.collection == collection)
            .cloned();

        match found {
            None => Ok(None),
            Some(record) => {
                require_object(&record.id, &record.data)?;
                Ok(Some(record))
            }
        }
    }

    fn create(&self, collection: &str, data: Json) -> Result<Record, StorageError> {
        let id = RecordId::new();
        let at = clock::now();
        let record = Record {
            id,
            collection: collection.to_string(),
            // Verbatim and unvalidated, exactly as the SQLite adapter stores it.
            data,
            created_at: at,
            updated_at: at,
        };

        self.records().insert(id, record.clone());
        Ok(record)
    }

    fn update(&self, collection: &str, id: &RecordId, data: Json) -> Result<Record, StorageError> {
        let mut records = self.records();

        // `get_mut` hands out a mutable borrow of the entry, edited in place.
        match records.get_mut(id) {
            Some(record) if record.collection == collection => {
                record.data = data; // wholesale replacement, not a merge
                record.updated_at = clock::now(); // created_at is left alone
                Ok(record.clone())
            }
            // Either no such id, or it belongs to another collection.
            _ => Err(StorageError::not_found(collection, id)),
        }
    }

    fn delete(&self, collection: &str, id: &RecordId) -> Result<(), StorageError> {
        let mut records = self.records();

        // Checked before removing, because `remove` alone cannot tell "gone now"
        // from "was never here" in a way that names the collection.
        let present = records
            .get(id)
            .is_some_and(|record| record.collection == collection);
        if !present {
            return Err(StorageError::not_found(collection, id));
        }

        records.remove(id);
        Ok(())
    }

    fn find_by_field(
        &self,
        collection: &str,
        field: &str,
        value: &Json,
    ) -> Result<Vec<Record>, StorageError> {
        let field = checked_field(field)?;
        let wanted = JsonKey::of(Some(value));

        let matched: Vec<Record> = self
            .records()
            .values()
            .filter(|record| record.collection == collection)
            // `JsonKey` equality is SQL's, so `1` matches `1.0` and a null value
            // matches an absent field — the null-safe `IS` of the SQL adapter.
            .filter(|record| JsonKey::at(&record.data, field) == wanted)
            .cloned()
            .collect();

        // Selected rows are materialised, so now corruption counts.
        for record in &matched {
            require_object(&record.id, &record.data)?;
        }
        Ok(matched) // already in id order: the map is keyed by ULID
    }
}
