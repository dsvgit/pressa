# SPEC-003: Storage and the record repository

Status: **Draft** · Task: T4 · Crate: `pressa-storage`
· ADRs: [0002](../docs/adr/0002-synchronous-rusqlite.md), [0003](../docs/adr/0003-json-documents-not-eav.md)

## Problem

Everything above storage is written against the `RecordRepository` port. If the
SQLite and in-memory implementations differ in any observable way, tests pass
against memory and the product breaks against the file.

## Goal

Two implementations of `RecordRepository` — `SqliteRepository` and
`MemoryRepository` — that are indistinguishable through the trait, verified by
one shared contract test suite.

## Non-goals

Soft delete, transactions across records, connection pooling, FTS5, a query
language, Postgres.

## API

The trait is defined in `pressa-core`; see
[`domain-model.md`](../docs/domain-model.md) §9. `ListParams` and the SQL
mapping are specified in [`storage.md`](../docs/storage.md) §4.

## Database

Schema, indexes and the migration runner: [`storage.md`](../docs/storage.md)
§2–3. Migrations are embedded with `include_str!`, applied in one transaction
on open, and tracked in `meta.schema_version`.

Connection setup on open: `journal_mode = WAL`, `foreign_keys = ON`,
`busy_timeout = 5000`. One `Connection` behind a `Mutex`.

## Behaviour

- `create` generates a ULID, sets `created_at = updated_at = now()`, and stores
  `data` verbatim. It does **not** validate — validation belongs to the service
  layer, and a repository that validates cannot be used to repair bad data.
- `update` replaces `data` wholesale (not a merge), sets `updated_at`, and
  leaves `created_at` untouched.
- `update` and `delete` on a missing id return `StorageError::NotFound`, never
  a silent success.
- `list` with default params returns records of that collection ordered by
  `id` ascending, which is chronological because ids are ULIDs.
- `list` never returns records from another collection.
- Sorting by a field appends `id` as a tiebreaker so the order is total.
- `search` matches case-insensitively as a substring against the fields named
  in the params.
- `find_by_field` compares the JSON value at `$.<field>` for equality.
- A row whose `data` is not a JSON object yields `StorageError::Corrupt` with
  the id, rather than panicking or being skipped.

`MemoryRepository` must reproduce all of the above, including `NotFound`,
ordering and `Corrupt`.

## Acceptance criteria

- [ ] A shared contract suite runs against both implementations and passes.
- [ ] Opening a fresh file creates the schema; opening it again is a no-op.
- [ ] Opening a database from a future `schema_version` fails with a clear error.
- [ ] Create → get returns an equal record.
- [ ] Create assigns a ULID and equal `created_at` / `updated_at`.
- [ ] Update changes `data` and `updated_at`, preserves `created_at` and `id`.
- [ ] Update on a missing id → `NotFound`.
- [ ] Delete removes the record; get then returns `Ok(None)`.
- [ ] Delete on a missing id → `NotFound`.
- [ ] List returns only the requested collection.
- [ ] List honours `limit` and `offset`.
- [ ] Sort by a field, ascending and descending, is correct and stable across
      repeated calls with equal keys.
- [ ] Search matches case-insensitively across the named fields only.
- [ ] `find_by_field` returns every match and an empty vector for none.
- [ ] A corrupt row yields `Corrupt` naming the id.
- [ ] Data written, connection dropped, database reopened → the data is there.

## Tests

- `tests/storage_contract.rs`: one suite, generic over `impl RecordRepository`,
  instantiated twice. New behaviour is added here, never to one implementation's
  own tests.
- SQLite tests use a `tempfile::TempDir`; no test touches a fixed path.
- One explicit reopen test — the Golden Path's step 13 depends on it.

## Open questions

None.
