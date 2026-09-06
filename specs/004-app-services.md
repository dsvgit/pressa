# SPEC-004: Application services

Status: **Draft** · Task: T5 · Crate: `pressa-app`

## Problem

The UI needs one place to ask for work that already knows the schema, runs
validation, enforces uniqueness and returns errors the UI can render. Without
it, that logic ends up in `update()`, where it is untestable and duplicated
between the create and edit paths.

## Goal

`RecordService` and `CollectionService`: the complete business API of Pressa,
and the only thing `pressa-tui` calls.

## Non-goals

Caching, pagination state, sorting policy, undo. Those are UI concerns and
belong in `AppState`.

## API

```rust
pub struct RecordService<R: RecordRepository> { repo: R, schema: Schema }

impl<R: RecordRepository> RecordService<R> {
    pub fn list(&self, collection: &str, params: &ListParams) -> Result<Vec<Record>, AppError>;
    pub fn count(&self, collection: &str, params: &ListParams) -> Result<u64, AppError>;
    pub fn get(&self, collection: &str, id: &RecordId) -> Result<Record, AppError>;
    pub fn create(&self, collection: &str, data: Value) -> Result<Record, AppError>;
    pub fn update(&self, collection: &str, id: &RecordId, data: Value) -> Result<Record, AppError>;
    pub fn delete(&self, collection: &str, id: &RecordId) -> Result<(), AppError>;
    /// Field defaults for a new record, from the schema.
    pub fn blank(&self, collection: &str) -> Result<Value, AppError>;
}

pub struct CollectionService { schema: Schema }

impl CollectionService {
    pub fn all(&self) -> impl Iterator<Item = &Collection>;
    pub fn get(&self, slug: &str) -> Result<&Collection, AppError>;
}

pub enum AppError {
    UnknownCollection(String),
    NotFound { collection: String, id: String },
    Validation(Vec<FieldError>),
    Config(#[from] ConfigError),
    Storage(#[from] StorageError),
}
```

`AppError::Validation` carries the structured errors through unchanged — the
editor needs them per-field, so flattening them to a string here would break
the form.

## Behaviour

Every method resolves the collection through the schema first; an unknown slug
is `UnknownCollection` and never reaches the repository.

`create` and `update`:

1. `validate_record(collection, &data)` — on failure, return
   `AppError::Validation` with those errors and **do not touch the repository**;
2. for each `unique: true` field with a present value, `repo.find_by_field`;
   any match with a different id appends a `NotUnique` error for that field;
3. if any uniqueness errors accumulated, return `Validation` with them;
4. otherwise call the repository.

`blank` returns an object with every field key present: `false` for `Boolean`,
the first option for a required `Select`, and `null` otherwise. A new record
form therefore starts from a document that has the right shape, so the editor
never has to invent one.

## Invariants

- An invalid record never reaches the repository. This is the invariant the
  whole storage design ([ADR-0003](../docs/adr/0003-json-documents-not-eav.md))
  depends on.
- Validation and uniqueness errors are returned together where possible, so one
  save attempt reports everything wrong.
- Services hold the schema by value and never re-read `pressa.yaml`.

## Acceptance criteria

- [ ] `create` with a valid document stores and returns a record with an id.
- [ ] `create` with an invalid document returns `Validation` and writes nothing
      (asserted by a subsequent `count` of 0).
- [ ] `create` with a duplicate value in a `unique` field returns `Validation`
      containing `NotUnique` for that field.
- [ ] `update` of a record with its own unique value does **not** report
      `NotUnique` against itself.
- [ ] `update` of a missing id returns `NotFound`.
- [ ] Any method with an unknown collection returns `UnknownCollection` without
      calling the repository.
- [ ] `blank` returns every field key, with `false` for booleans and the first
      option for required selects.
- [ ] `delete` removes the record; a second `delete` returns `NotFound`.
- [ ] Integration test: load schema → create → list → get → update → delete,
      against `SqliteRepository` on a temp file.

## Tests

Service tests use `MemoryRepository`; the integration test above uses
`SqliteRepository`. A mock repository that records calls verifies the "never
reaches the repository" criteria.

## Open questions

None.
