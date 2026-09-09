-- Storage engine schema, version 1. See docs/storage.md §2.
--
-- These migrations version the engine's own tables, never the user's
-- collections: editing pressa.yaml must never produce a migration, which is
-- the entire point of ADR-0003.

CREATE TABLE IF NOT EXISTS records (
    id          TEXT PRIMARY KEY,       -- ULID
    collection  TEXT NOT NULL,
    data        TEXT NOT NULL,          -- JSON object
    created_at  TEXT NOT NULL,          -- RFC 3339
    updated_at  TEXT NOT NULL
);

-- Covers the default query — one collection ordered by creation time — without
-- a second sort step, because ULIDs sort chronologically.
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
