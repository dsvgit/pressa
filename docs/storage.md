# Storage

Status: Approved · Last updated: 2026-09-06 · Crate: `pressa-storage`

## 1. Shape of the decision

Records are stored as **JSON documents in one generic table**, not as a
relational table per collection and not as EAV. The reasoning and the rejected
alternatives are in [ADR-0003](adr/0003-json-documents-not-eav.md).

Short version: collections are user-defined and change while the app is being
used, so a table-per-collection design turns every schema edit into a
migration. EAV avoids that but destroys filtering, sorting, indexing and
readability. JSON documents in SQLite keep schema changes free and stay
queryable through SQLite's JSON functions.

## 2. Database schema

`migrations/0001_init.sql`:

```sql
CREATE TABLE IF NOT EXISTS records (
    id          TEXT PRIMARY KEY,       -- ULID
    collection  TEXT NOT NULL,
    data        TEXT NOT NULL,          -- JSON object
    created_at  TEXT NOT NULL,          -- RFC 3339
    updated_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_records_collection
    ON records(collection, id);

-- Snapshot of the collection config that was active when records were written.
-- Not read in M0; it is what makes future schema migrations diffable.
CREATE TABLE IF NOT EXISTS collections (
    slug        TEXT PRIMARY KEY,
    config      TEXT NOT NULL,          -- JSON dump of the Collection
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meta (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL
);
```

The composite index `(collection, id)` covers the default query — list one
collection ordered by creation time — without a second sort step.

`collections` is written on every startup and never read in M0. It costs one
upsert per collection per launch and buys the ability, in M3, to detect that a
field was renamed or removed since the data was written. Do not delete it as
"dead code".

`meta` holds `schema_version` for the migration runner.

## 3. Migrations

A hand-rolled runner, not a crate dependency:

- `migrations/NNNN_name.sql` files, embedded with `include_str!`;
- `meta.schema_version` records the highest applied number;
- on open, apply every migration above the stored version inside one
  transaction, then update `schema_version`;
- forward-only. No down migrations — this is a local file the user can delete.

This is roughly forty lines. `refinery` would be a dependency and an ADR for
less.

Note the distinction, because it causes confusion: these migrations version the
**storage engine's own tables**, not the user's collections. Changing
`pressa.yaml` never produces a migration; that is the entire point of
[ADR-0003](adr/0003-json-documents-not-eav.md).

## 4. ListParams

```rust
pub struct ListParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Case-insensitive substring match, applied across `search_fields`.
    pub search: Option<String>,
    /// The fields `search` looks in — normally the collection's list_columns.
    pub search_fields: Vec<String>,
    /// Field name; None means order by id (chronological).
    pub sort_by: Option<String>,
    pub sort_direction: SortDirection,
}

pub enum SortDirection { Asc, Desc }
```

`Default` is: no limit, no offset, no search, sort by id ascending.

`search_fields` is named here rather than derived from the schema because the
repository has no schema: `list` is handed a collection *slug*, not a
`Collection`. The caller — `RecordService` — passes the collection's
`list_columns`. A search with no field named matches nothing.

### Mapping to SQL

Base:

```sql
SELECT id, collection, data, created_at, updated_at
FROM records
WHERE collection = ?1
ORDER BY id ASC
LIMIT ?2 OFFSET ?3;
```

With `sort_by = Some("title")`:

```sql
ORDER BY json_extract(data, '$.title') ASC, id ASC
```

The trailing `id` makes the order total, so a sorted list never reshuffles rows
with equal keys between renders — snapshot tests would otherwise be flaky.

With `search = Some("hello")` over `list_columns = [title, status]`:

```sql
AND (lower(json_extract(data, '$.title'))  LIKE '%hello%'
  OR lower(json_extract(data, '$.status')) LIKE '%hello%')
```

The search terms are bound as parameters; the *column list* is interpolated
from `list_columns`, which is why field names are validated against
`^[a-z][a-z0-9_]*$` at schema load time. That validation is a security
boundary, not cosmetics — never relax it without revisiting this query.

M0 uses `LIKE`. FTS5 is a later optimisation and needs its own ADR.

## 5. Implementations

### `SqliteRepository`

- `rusqlite` with the `bundled` feature — no system SQLite dependency, so
  `cargo build` works on a clean machine and in CI.
- Pragmas on open: `journal_mode = WAL`, `foreign_keys = ON`,
  `busy_timeout = 5000`.
- One `Connection` behind a `Mutex`. A single-user local TUI has no concurrency
  to speak of; a pool would be complexity without a problem.
- `create` and `update` set `created_at` / `updated_at` themselves. The service
  layer does not pass timestamps — that keeps "when did this change" a single
  source of truth.
- `update` on a missing id is `StorageError::NotFound`, not a silent no-op.

### `MemoryRepository`

An in-memory `BTreeMap<RecordId, Record>` implementing the same trait. A tree
rather than the insertion-ordered map first specified here: keys are ULIDs, so
tree order *is* id order — the default list order — and that holds even for rows
inserted out of sequence, which insertion order would not. It
exists so that `pressa-app` and `pressa-tui` tests never open a file, and so
that a failing service test cannot be blamed on SQL. It must implement the
*same* semantics, including `NotFound` on missing ids and the same sort order —
a divergence here produces tests that pass against memory and fail in
production.

Both implementations are exercised by a shared test suite (`storage_contract`)
parameterised over the trait, so the two can never drift apart silently.

## 6. Errors

```rust
pub enum StorageError {
    NotFound { collection: String, id: String },
    UnknownCollection(String),
    Corrupt { id: String, detail: String },   // stored JSON is not an object
    InvalidField { field: String },           // would be interpolated into SQL
    Migration { detail: String },             // schema_version this build cannot read
    Io(String),
    Sqlite(String),                           // a backend error, flattened
}
```

`rusqlite::Error` is wrapped, never re-exported: `pressa-app` and `pressa-tui`
must be able to handle storage failures without depending on rusqlite. That is
why `Sqlite` carries a `String` and not `#[from] rusqlite::Error` — the type
is named in the port's signatures, and the port lives in `pressa-core`, which
must never know rusqlite exists (`AGENTS.md`, `docs/architecture.md` §1). So the
enum lives in `pressa-core` alongside the trait that returns it, and the adapter
flattens its backend errors on the way out.

`InvalidField` is the backstop on the one place a field name reaches SQL by
interpolation rather than binding (§4). Schema loading rejects such names first;
storage refuses them again rather than trusting its caller.

## 7. What we are not doing in M0

- No soft delete (`deleted_at`) — deletion is deletion.
- No `unique_values` side table. Uniqueness is checked with `find_by_field`
  before write. That is a race in theory and irrelevant in a single-user local
  app; when it stops being irrelevant, it needs an ADR and a real constraint.
- No transactions spanning multiple records — every operation touches one row.
- No connection pooling, no async, no Postgres.
