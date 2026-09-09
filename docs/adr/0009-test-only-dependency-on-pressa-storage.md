# ADR-0009: `pressa-app` may depend on `pressa-storage` in tests only

Status: **Accepted** · Date: 2026-09-09

## Context

`RecordService` is generic over `RecordRepository`, so testing it needs *some*
implementation of that port. Two exist, and both live in `pressa-storage`:
`MemoryRepository` and `SqliteRepository`.

Three documents disagreed about whether `pressa-app` may reach them from its
tests, and the conflict surfaced while implementing T5.

1. [SPEC-004](../../specs/004-app-services.md) "Tests" says: "Service tests use
   `MemoryRepository`; the integration test above uses `SqliteRepository`." Its
   last acceptance criterion requires a lifecycle test "against
   `SqliteRepository` on a temp file".
2. [`pressa-storage/src/memory.rs`](../../pressa-storage/src/memory.rs) says
   `MemoryRepository` "exists so `pressa-app` and `pressa-tui` tests never open
   a database" — its stated purpose is to be used from `pressa-app` tests.
3. [`architecture.md`](../architecture.md) §2 listed `pressa-storage` under
   `pressa-app`'s "must not depend on", and
   [`pressa-tui/tests/architecture.rs`](../../pressa-tui/tests/architecture.rs)
   enforced it by scanning `Cargo.toml` — including `[dev-dependencies]`,
   deliberately.

So (1) and (2) asked for a test-only dependency that (3) forbade. The conflict
was never about production code: no `pressa-app` source file names storage, and
the production dependency graph is identical either way.

## Decision

`pressa-app` may name `pressa-storage` in `[dev-dependencies]`, and nowhere
else. `rusqlite` remains forbidden to `pressa-app` in **every** section,
including dev.

The guard enforces exactly that: the `pressa-app` → `pressa-storage` check
ignores dev sections, every other edge keeps scanning all of them.

## Rationale

- The rule worth enforcing is "no SQLite in the business layer", and forbidding
  `rusqlite` in every section already secures it. Forbidding a test-only path to
  the port's own in-memory fake enforces something stricter than the rule's
  purpose without protecting anything more.
- It keeps `pressa-app`'s tests in `pressa-app`, so `cargo test -p pressa-app`
  covers the services it owns.
- `MemoryRepository` is held to the port contract by the shared suite in
  `specs/003`. A fake hand-maintained inside `pressa-app/tests/` would be a
  second implementation of the port with nothing holding it to that contract,
  free to drift until service tests pass against behaviour SQLite does not have.

## Alternatives considered

- **Service tests in `pressa-tui/tests/`.** No rule change — the composition
  root may depend on both — but `pressa-app`'s tests would live in another
  crate and `cargo test -p pressa-app` would stop covering the services.
- **A hand-written fake in `pressa-app/tests/`.** Re-implements the thing
  `MemoryRepository` exists to be, with the drift risk above, and the
  `SqliteRepository` criterion still could not be met in this crate.
- **A `test-support` feature on `pressa-core`.** The port lives there, so a
  reference fake could too, and every layer above could use it. Rejected as the
  largest change, putting test code in the domain crate, for a seam nothing has
  asked for yet. Worth revisiting if `pressa-tui` later wants the same fake.

## Consequences

- `pressa-app/Cargo.toml` carries `pressa-storage` under `[dev-dependencies]`.
  `cargo test -p pressa-app` therefore compiles rusqlite; `cargo build` does not.
- The guard grows one documented exception and one test for it
  (`a_test_only_dependency_on_storage_is_allowed_but_a_real_one_is_not`). The
  existing `the_scanner_sees_a_dependency_however_it_is_written` is unchanged
  and still proves the default scanner sees dev sections.
- The exception is one edge wide. It is not a precedent for `pressa-core`, which
  may not name any sibling crate in any section.
