# Architecture

Status: Approved · Last updated: 2026-09-06

## 1. The dependency rule

```
        ┌──────────────┐
        │  pressa-tui  │   binary `pressa`: CLI, event loop, screens, widgets
        └──────┬───────┘
               │ calls services, never SQL
        ┌──────▼───────┐
        │  pressa-app  │   use cases: RecordService, CollectionService
        └──────┬───────┘
               │
        ┌──────▼───────┐              ┌────────────────┐
        │ pressa-core  │◀─────────────│ pressa-storage │
        │              │  implements  │                │
        │ Schema       │  Repository  │ SqliteRepository│
        │ Record/Value │              │ MemoryRepository│
        │ Validation   │              │ migrations      │
        │ Repository   │              └────────────────┘
        │  (port)      │
        └──────────────┘
```

One rule, non-negotiable:

> **`pressa-tui` must never touch SQLite, and `pressa-core` must never know
> that ratatui or rusqlite exist.**

`pressa-core` defines the `RecordRepository` *port*. `pressa-storage` provides
the *adapter*. Dependencies point inward; the arrow from storage to core is an
implementation of a trait owned by core, not a dependency of core on storage.

## 2. Why a workspace, and why exactly four crates

Multi-crate boundaries are not ceremony here — they are the enforcement
mechanism. In a single crate, "the TUI must not open the database" is a rule an
agent can forget. Across crates it is a compile error, because `pressa-tui` has
no `rusqlite` dependency to reach for.

That is the entire justification: **the compiler reviews the architecture so a
human doesn't have to.** See [ADR-0006](adr/0006-workspace-of-four-crates.md).

| Crate | Owns | Depends on | Must not depend on |
|---|---|---|---|
| `pressa-core` | `Schema`, `Collection`, `Field`, `FieldType`, `Value`, `Record`, validation, `RecordRepository` trait, domain errors | `serde`, `serde_json`, `thiserror`, `chrono`, `ulid`, `indexmap` | ratatui, crossterm, rusqlite, clap, tokio |
| `pressa-storage` | `SqliteRepository`, `MemoryRepository`, migration runner, `StorageError` | `pressa-core`, `rusqlite` | ratatui, crossterm, clap |
| `pressa-app` | `RecordService`, `CollectionService`, config loading, `AppError` | `pressa-core`, `serde_yaml` | ratatui, crossterm, rusqlite |
| `pressa-tui` | binary `pressa`: clap CLI, terminal lifecycle, `AppState`, `Command`, keymap, screens, widgets, tracing setup | `pressa-app`, `pressa-storage` (wiring only, in `main.rs`), `ratatui`, `crossterm`, `clap` | — |

`pressa-tui` is allowed to name `pressa-storage` in exactly one place: the
composition root in `main.rs`, where a `SqliteRepository` is constructed and
handed to the services. Everywhere else it sees only `pressa-app` types.

## 3. Data flow

### 3.1 Startup

```
main.rs
  → clap parses argv
  → tracing subscriber → .pressa/pressa.log   (never stdout: the TUI owns it)
  → load pressa.yaml            → Schema          (pressa-app)
  → open .pressa/data.db, run migrations         (pressa-storage)
  → build RecordService / CollectionService      (pressa-app)
  → enter terminal raw mode + alternate screen
  → run event loop
```

### 3.2 The loop

```
crossterm event
   → Keymap lookup (context = current Screen)
   → Command
   → update(&mut AppState, Command) -> Vec<Effect>     ← pure, no I/O
   → run_effects(Effect, &services) -> Vec<Command>    ← the only I/O
   → view(&AppState, frame)                            ← pure render
```

`update` is a pure function and is the thing we unit-test. `Effect` is the
narrow, enumerable list of everything the UI is allowed to ask the world to do
(`LoadRecords`, `SaveRecord`, `DeleteRecord`, …). Keeping I/O out of `update`
is what makes UI logic testable without a database or a terminal.

See [`tui.md`](tui.md) for the state model in detail.

## 4. Errors

Three layers, three error types, converted at the boundary:

- `pressa-core`: `SchemaError`, `ValidationError` — `thiserror`, no I/O concepts.
- `pressa-storage`: `StorageError` — wraps `rusqlite::Error`, never leaks it.
- `pressa-app`: `AppError` — `#[from]` both of the above; this is what the TUI
  sees and renders.

`anyhow` is permitted only in `main.rs` and in tests. No `unwrap()` or
`expect()` in library code outside tests.

## 5. Configuration and project layout

A Pressa *project* is a directory:

```
my-project/
├── pressa.yaml          # the schema — the user edits this
└── .pressa/
    ├── data.db          # SQLite
    └── pressa.log       # tracing output
```

`pressa.yaml` is discovered by walking up from the current directory, the way
`git` finds `.git`. `.pressa/` lives next to the file that was found.

## 6. Extension points reserved but not built

These shape the design without being implemented in M0:

- **`RecordRepository` as a port** — a `PostgresRepository` or `ApiRepository`
  later requires no change above the storage crate.
- **`Command` enum as the whole user-action surface** — a command palette (M1),
  custom keybindings, or a vim mode are additive, not rewrites.
- **Services as the business API** — a REST layer or an MCP server (M5) would
  sit beside the TUI on top of `pressa-app`, not inside it.
- **`CollectionCapabilities`** — a per-collection `{create, read, update,
  delete}` struct, so future read-only or computed collections do not require
  the UI to special-case them. Defined in M0, always all-true, checked by the
  UI when enabling commands.

None of these are built now. They are the reason the boundaries sit where they
sit.

## 7. Related decisions

- [ADR-0001](adr/0001-ratatui-and-crossterm.md) — ratatui + crossterm
- [ADR-0002](adr/0002-synchronous-rusqlite.md) — synchronous rusqlite, no tokio
- [ADR-0003](adr/0003-json-documents-not-eav.md) — JSON documents, not EAV
- [ADR-0004](adr/0004-schema-driven-ui.md) — schema-driven UI
- [ADR-0005](adr/0005-command-and-keymap-architecture.md) — command/keymap layer
- [ADR-0006](adr/0006-workspace-of-four-crates.md) — workspace of four crates
- [ADR-0007](adr/0007-vertical-slice-first.md) — vertical slice before breadth
