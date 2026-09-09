//! [`RecordService`]: the only way records are created, read, changed or removed.
//!
//! Every method resolves the collection through the schema before it touches
//! storage, and `create`/`update` validate before they write — the invariant
//! [ADR-0003](../../../docs/adr/0003-json-documents-not-eav.md) depends on,
//! since SQLite enforces no types of its own.

use serde_json::{Map, Value as Json};

use pressa_core::record::{Record, RecordId};
use pressa_core::repository::{ListParams, RecordRepository, StorageError};
use pressa_core::schema::{Collection, FieldType, Schema};
use pressa_core::validation::{ErrorCode, FieldError, validate_record};

use crate::error::AppError;

/// The `NotUnique` message, fixed by SPEC-002's error table.
const ALREADY_USED: &str = "already used by another record";

/// Records, with the schema applied. Generic over the repository so the same
/// service is tested against `MemoryRepository` and run against SQLite.
pub struct RecordService<R: RecordRepository> {
    repo: R,
    schema: Schema,
}

impl<R: RecordRepository> RecordService<R> {
    /// Takes the schema by value: services never re-read `pressa.yaml`.
    pub fn new(repo: R, schema: Schema) -> Self {
        RecordService { repo, schema }
    }

    /// The collection for `slug`, or `UnknownCollection` — the first thing every
    /// method does, so an unknown slug never reaches the repository.
    fn collection(&self, slug: &str) -> Result<&Collection, AppError> {
        self.schema
            .collections
            .get(slug)
            // `ok_or_else` builds the error only when the lookup missed.
            .ok_or_else(|| AppError::UnknownCollection(slug.to_string()))
    }

    pub fn list(&self, collection: &str, params: &ListParams) -> Result<Vec<Record>, AppError> {
        let collection = self.collection(collection)?;
        self.repo
            .list(&collection.slug, params)
            .map_err(from_storage)
    }

    pub fn count(&self, collection: &str, params: &ListParams) -> Result<u64, AppError> {
        let collection = self.collection(collection)?;
        self.repo
            .count(&collection.slug, params)
            .map_err(from_storage)
    }

    pub fn get(&self, collection: &str, id: &RecordId) -> Result<Record, AppError> {
        let collection = self.collection(collection)?;

        // The port reports a missing record as `Ok(None)`; the service's callers
        // want an error they can render, so absence is translated here.
        match self.repo.get(&collection.slug, id).map_err(from_storage)? {
            Some(record) => Ok(record),
            None => Err(not_found(&collection.slug, id)),
        }
    }

    pub fn create(&self, collection: &str, data: Json) -> Result<Record, AppError> {
        let collection = self.collection(collection)?;
        self.check(collection, &data, None)?;

        self.repo
            .create(&collection.slug, data)
            .map_err(from_storage)
    }

    pub fn update(&self, collection: &str, id: &RecordId, data: Json) -> Result<Record, AppError> {
        let collection = self.collection(collection)?;
        // `Some(id)` so the record's own value is not a duplicate of itself.
        self.check(collection, &data, Some(id))?;

        self.repo
            .update(&collection.slug, id, data)
            .map_err(from_storage)
    }

    pub fn delete(&self, collection: &str, id: &RecordId) -> Result<(), AppError> {
        let collection = self.collection(collection)?;
        self.repo.delete(&collection.slug, id).map_err(from_storage)
    }

    /// Field defaults for a new record, from the schema.
    ///
    /// Every field key is present, so the editor never has to invent the shape
    /// of a document. A required `Select` is left empty on purpose: pre-filling
    /// a field the user never answered would let the blank form pass validation
    /// (SPEC-004 "Behaviour").
    pub fn blank(&self, collection: &str) -> Result<Json, AppError> {
        let collection = self.collection(collection)?;

        let mut document = Map::new();
        for field in &collection.fields {
            // A boolean has no "unset": an unchecked box is `false`.
            let value = if matches!(field.kind, FieldType::Boolean) {
                Json::Bool(false)
            } else {
                Json::Null
            };
            document.insert(field.name.clone(), value);
        }
        Ok(Json::Object(document))
    }

    /// Everything that must hold before a write, in SPEC-004's order.
    ///
    /// Schema validation first and on its own: an invalid document must not
    /// reach the repository at all, not even the `find_by_field` of the
    /// uniqueness check. `id` is the record being updated, if any.
    fn check(
        &self,
        collection: &Collection,
        data: &Json,
        id: Option<&RecordId>,
    ) -> Result<(), AppError> {
        // `map_err` wraps the field errors unchanged — the editor needs them per field.
        validate_record(collection, data).map_err(AppError::Validation)?;

        let conflicts = self.uniqueness_errors(collection, data, id)?;
        if !conflicts.is_empty() {
            return Err(AppError::Validation(conflicts));
        }
        Ok(())
    }

    /// A `NotUnique` error for every `unique` field whose value another record
    /// already holds. All of them, so one save reports everything wrong.
    fn uniqueness_errors(
        &self,
        collection: &Collection,
        data: &Json,
        id: Option<&RecordId>,
    ) -> Result<Vec<FieldError>, AppError> {
        let mut errors = Vec::new();

        // `filter` narrows the borrowed fields to the ones the rule applies to.
        for field in collection.fields.iter().filter(|field| field.unique) {
            // Absent and null are both "no value", and two of those never
            // collide (SPEC-002 "Uniqueness"). `find_by_field` would match them
            // against each other, so they are skipped before it is called.
            let Some(value) = data
                .get(field.name.as_str())
                .filter(|value| !value.is_null())
            else {
                continue;
            };

            let matched = self
                .repo
                .find_by_field(&collection.slug, &field.name, value)
                .map_err(from_storage)?;

            // `any` looks for a match that is some *other* record; comparing
            // `Option<&RecordId>` makes "no id yet" differ from every stored id.
            if matched.iter().any(|record| Some(&record.id) != id) {
                errors.push(FieldError {
                    field: field.name.clone(),
                    code: ErrorCode::NotUnique,
                    message: ALREADY_USED.to_string(),
                });
            }
        }
        Ok(errors)
    }
}

/// The `NotFound` a missing id produces, phrased like `StorageError`'s.
fn not_found(collection: &str, id: &RecordId) -> AppError {
    AppError::NotFound {
        collection: collection.to_string(),
        id: id.as_string(),
    }
}

/// Translates a storage failure into the app's vocabulary.
///
/// Repository calls go through this rather than a bare `?`: `AppError` derives
/// `#[from] StorageError`, so `?` alone would bury a missing record inside
/// `AppError::Storage` when callers are promised `AppError::NotFound`.
fn from_storage(error: StorageError) -> AppError {
    match error {
        StorageError::NotFound { collection, id } => AppError::NotFound { collection, id },
        // Everything else keeps its own vocabulary. `StorageError::UnknownCollection`
        // is deliberately not translated: no adapter ever constructs it, so an arm
        // for it would be a branch no test could reach.
        other => AppError::Storage(other),
    }
}
