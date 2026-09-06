# ADR-0006: A Cargo workspace of four crates

Status: **Accepted** · Date: 2026-09-06

## Context

Three source plans proposed a workspace of three to five crates; `ai/4.md`
argued for a single crate with modules (`src/domain`, `src/application`,
`src/tui`) on the grounds that a workspace is premature before real boundaries
exist.

That argument is right for a human team and wrong here, for one reason: most of
this code will be written by AI agents, and a module boundary is a rule an
agent can forget while a crate boundary is a compile error.

## Decision

A Cargo workspace with four crates:

```
pressa-core      domain, validation, RecordRepository port
pressa-storage   SqliteRepository, MemoryRepository, migrations
pressa-app       RecordService, CollectionService, config loading
pressa-tui       binary `pressa`: CLI, event loop, screens, widgets
```

## Rationale

- "The TUI must not touch SQLite" is enforced by `pressa-tui` not depending on
  `rusqlite`, rather than by review. The compiler is a reviewer that never gets
  tired and never approves a Friday PR.
- Boundaries also bound *context*: an agent working on T4 needs `pressa-core`
  and `pressa-storage`, not the UI. Smaller context, better output.
- Unit tests for the domain compile and run without ratatui or SQLite in the
  build graph.
- The dependency direction is visible in `Cargo.toml` files, so a violation
  shows up in the diff as a new dependency line — the most reviewable possible
  form of an architecture violation.

Four rather than five: the CLI is thin (three clap subcommands) and shares the
binary with the TUI, so a separate `pressa-cli` crate would be a crate per file.
It can be split out later if the CLI grows.

## Consequences

- More `Cargo.toml` files and slightly more friction moving a type between
  layers. That friction is a signal, not a cost — it asks whether the type is
  in the right layer.
- `pressa-tui` is allowed to name `pressa-storage` in exactly one place: the
  composition root in `main.rs`. Anywhere else is a violation.
- Any new inter-crate dependency edge requires an ADR.
