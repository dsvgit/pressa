# ADR-0003: Store records as JSON documents in one table

Status: **Accepted** · Date: 2026-09-06

## Context

Collections are defined by the user in `pressa.yaml` and change while the
application is in use. Three storage layouts were considered:

1. **A relational table per collection.** Strong typing, real indexes and
   constraints — and every edit to `pressa.yaml` becomes a schema migration,
   including during the editing session that made it.
2. **EAV** (`record_id`, `field_name`, `field_value`). Fully dynamic, and it
   destroys filtering, sorting, indexing, constraints and readability. Every
   source plan that mentioned it rejected it.
3. **JSON documents in one generic table.** Schema changes are free; SQLite's
   JSON functions keep the data queryable.

## Decision

One `records` table: `id`, `collection`, `data` (JSON object), `created_at`,
`updated_at`. Alongside it, a `collections` table holding a JSON snapshot of
each collection's config, written on startup and not read in M0.

## Rationale

- Adding a field, renaming a label or introducing a whole collection requires
  no migration. That is the property that makes the schema-driven premise of
  the product actually work.
- SQLite's `json_extract` supports the queries M0 needs — filter by collection,
  sort by a field, substring-search across the list columns.
- The `collections` snapshot costs one upsert per launch and is what makes M3's
  schema diffing possible: without a record of what the schema was when the
  data was written, "this field was renamed" is unknowable.

## Consequences

- No database-level typing. Validation in `pressa-core` is the only guarantee
  that stored data matches the schema — so the validator is load-bearing and is
  specified separately in `specs/002`.
- No database-level uniqueness. `unique: true` is enforced by a `find_by_field`
  check before writing. This is racy in theory and irrelevant for a single-user
  local application; making it real needs a new ADR.
- Sorting and searching go through `json_extract`, which cannot use an ordinary
  index. At MVP data sizes this is fine. Generated columns with indexes are the
  escape hatch if it stops being fine.
- Large collections will eventually want a relational mode
  (`storage.mode: relational`). The `RecordRepository` port is where that would
  live, and nothing above storage would change.
