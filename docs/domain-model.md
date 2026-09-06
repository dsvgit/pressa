# Domain Model

Status: Approved · Last updated: 2026-09-06 · Crate: `pressa-core`

All types below live in `pressa-core` and depend on nothing but `serde`,
`serde_json`, `thiserror`, `chrono`, `ulid` and `indexmap`.

## 1. Overview

```
Schema
 └── Collection            (slug: "posts")
      ├── Field            (name: "title", kind: Text, required: true)
      ├── Field
      └── list_columns     which fields appear in the table view

Record                     one document belonging to one collection
 └── data: serde_json::Value
```

## 2. Schema

```rust
pub struct Schema {
    pub project: ProjectConfig,
    pub database: DatabaseConfig,
    /// Insertion order is preserved: it is the sidebar order.
    pub collections: IndexMap<String, Collection>,
}

pub struct ProjectConfig {
    pub name: String,
}

pub struct DatabaseConfig {
    /// Relative to the directory containing pressa.yaml. Default ".pressa/data.db".
    pub path: PathBuf,
}
```

`IndexMap` rather than `HashMap` is deliberate: the order the user wrote their
collections in is the order they see in the sidebar, and a `HashMap` would make
snapshot tests non-deterministic.

Lookup helpers:

```rust
impl Schema {
    pub fn collection(&self, slug: &str) -> Option<&Collection>;
    pub fn require_collection(&self, slug: &str) -> Result<&Collection, SchemaError>;
    pub fn collections(&self) -> impl Iterator<Item = &Collection>;
}
```

## 3. Collection

```rust
pub struct Collection {
    pub slug: String,                 // "posts" — stable id, used in storage
    pub label: String,                // "Posts" — shown in the UI
    pub fields: Vec<Field>,
    /// Field names shown as table columns. Defaults to the first 4 fields.
    pub list_columns: Vec<String>,
    pub capabilities: CollectionCapabilities,
}

pub struct CollectionCapabilities {
    pub create: bool,
    pub read: bool,
    pub update: bool,
    pub delete: bool,
}
```

In M0 every capability is `true`. It exists so that read-only collections later
do not require the UI to grow special cases; the UI already asks
`collection.capabilities.delete` before offering `d`.

Collection invariants (enforced at load time, see [`specs/001`](../specs/001-schema-config.md)):

- `slug` matches `^[a-z][a-z0-9_]*$` and is unique in the schema;
- at least one field;
- field names unique within the collection;
- every entry in `list_columns` names an existing field.

## 4. Field

```rust
pub struct Field {
    pub name: String,                 // "title"
    pub label: String,                // "Title" — defaults to titlecased name
    pub kind: FieldType,
    pub required: bool,
    pub unique: bool,
}
```

## 5. FieldType

```rust
pub enum FieldType {
    Text,
    Textarea,
    Number,
    Boolean,
    DateTime,
    Select { options: Vec<String> },
    Json,
}
```

**Seven types in M0.** `Relation` is explicitly deferred to M2: it is the only
field type that needs to read another collection and open a picker overlay, so
it drags a second data path and a new screen into the MVP. The other six are
self-contained.

Dispatch is by `match` on this enum, **not** `Box<dyn Field>`. Trait objects
here would buy polymorphism we do not need and cost us exhaustiveness checking
— which is exactly the signal that tells an agent "you added a field type and
forgot the editor widget".

Each `FieldType` answers three questions, and every new type must answer all
three:

| Question | Where it is answered |
|---|---|
| How do I validate a value? | `pressa-core::validation` |
| How do I render in a table cell? | `pressa-tui::widgets::table` |
| How do I render and edit as a form field? | `pressa-tui::widgets::form` |

## 6. Value

```rust
pub enum Value {
    Null,
    Text(String),
    Number(f64),
    Boolean(bool),
    DateTime(DateTime<Utc>),
    Json(serde_json::Value),
}
```

`Value` is the *typed* view used by validation and the editors. The stored form
is always `serde_json::Value` — we do not write our own JSON implementation and
we do not persist `Value` directly. Conversion is explicit:

```rust
impl Value {
    pub fn from_json(kind: &FieldType, raw: &serde_json::Value) -> Result<Value, ValidationError>;
    pub fn to_json(&self) -> serde_json::Value;
}
```

`DateTime` is stored as an RFC 3339 string. `Number` is `f64`: SQLite stores
JSON numbers as doubles anyway, and a `Decimal` type would be precision theatre
until we have a use case that needs it.

## 7. Record

```rust
pub struct Record {
    pub id: RecordId,                     // ULID: sortable by creation time
    pub collection: String,
    pub data: serde_json::Value,          // always a JSON object
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct RecordId(Ulid);
```

ULID over UUIDv4 so that `ORDER BY id` is chronological and the default list
view is stable without an extra index.

`data` is the *whole* document as an object; fields absent from the object are
absent, not `null`. Validation decides whether that is acceptable.

## 8. Validation

```rust
pub struct FieldError {
    pub field: String,        // "title"
    pub code: ErrorCode,      // Required, TypeMismatch, NotInOptions, ...
    pub message: String,      // human-readable, rendered under the input
}

pub enum ErrorCode {
    Required,
    TypeMismatch,
    NotInOptions,
    UnknownField,
    NotUnique,
    InvalidDateTime,
    InvalidJson,
}

pub fn validate_record(
    collection: &Collection,
    data: &serde_json::Value,
) -> Result<(), Vec<FieldError>>;
```

Errors are a **vector of structured errors, never a string**. The form has to
place each message under the field it belongs to, and tests have to assert on
`code` rather than on prose.

`NotUnique` cannot be decided by `pressa-core` alone — it needs storage. The
core validator returns everything else; `RecordService` runs the uniqueness
check against the repository and appends `NotUnique` errors before saving. See
[`specs/002`](../specs/002-record-validation.md).

Validation rules per type are specified in
[`specs/002-record-validation.md`](../specs/002-record-validation.md) — that
spec, not this document, is the contract the tests are written against.

## 9. The repository port

Defined in `pressa-core`, implemented in `pressa-storage`:

```rust
pub trait RecordRepository {
    fn list(&self, collection: &str, params: &ListParams) -> Result<Vec<Record>, StorageError>;
    fn count(&self, collection: &str, params: &ListParams) -> Result<u64, StorageError>;
    fn get(&self, collection: &str, id: &RecordId) -> Result<Option<Record>, StorageError>;
    fn create(&self, collection: &str, data: serde_json::Value) -> Result<Record, StorageError>;
    fn update(&self, collection: &str, id: &RecordId, data: serde_json::Value) -> Result<Record, StorageError>;
    fn delete(&self, collection: &str, id: &RecordId) -> Result<(), StorageError>;
    fn find_by_field(&self, collection: &str, field: &str, value: &serde_json::Value) -> Result<Vec<Record>, StorageError>;
}
```

Synchronous by decision — see [ADR-0002](adr/0002-synchronous-rusqlite.md).
`find_by_field` exists to support uniqueness checks without giving the service
layer a query language it does not yet need.

Details of `ListParams` and the SQLite mapping: [`storage.md`](storage.md).
