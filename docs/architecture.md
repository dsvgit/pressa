# Architecture

Status: Approved · Last updated: 2026-09-09

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
| `pressa-core` | `Schema`, `Collection`, `Field`, `FieldType`, `Value`, `Record`, validation, `RecordRepository` trait, `ListParams`, domain errors including `StorageError` | `serde`, `serde_json`, `thiserror`, `chrono`, `ulid`, `indexmap` | ratatui, crossterm, rusqlite, clap, tokio |
| `pressa-storage` | `SqliteRepository`, `MemoryRepository`, migration runner | `pressa-core`, `rusqlite`, `serde_json`, `chrono` | ratatui, crossterm, clap |
| `pressa-app` | `RecordService`, `CollectionService`, config loading, `AppError` | `pressa-core`, `serde`, `serde_yaml`, `serde_json`, `indexmap`, `thiserror` | ratatui, crossterm, rusqlite, pressa-storage (outside `[dev-dependencies]` — [ADR-0009](adr/0009-test-only-dependency-on-pressa-storage.md)) |
| `pressa-tui` | binary `pressa`: clap CLI, terminal lifecycle, `AppState`, `Command`, keymap, screens, widgets, tracing setup | `pressa-app`, `pressa-storage` (wiring only, in `cli.rs`'s startup sequence), `ratatui`, `crossterm`, `clap`, `thiserror`, `tracing`, `tracing-subscriber`; `assert_cmd` and `predicates` in `[dev-dependencies]` ([ADR-0010](adr/0010-test-only-dependencies-need-an-adr.md)) | rusqlite |

`pressa-tui` is allowed to name `pressa-storage` in exactly one place: the
composition root, where a `SqliteRepository` is constructed and handed to the
services. Everywhere else it sees only `pressa-app` types.

Since T6 that place is `cli::dev` in [`cli.rs`](../pressa-tui/src/cli.rs), not
`main.rs`. `main.rs` is a shim — parse argv, read the working directory, call
`cli::run` — because a `main` cannot be called from a test, and SPEC-005 asks
for every path through the CLI to be exercised in-process
([SPEC-005](../specs/005-cli.md) "API"). The rule the boundary is there for is
unchanged: exactly one function names a storage type, and nothing above it
sees one.

`StorageError` lives in `pressa-core`, not in `pressa-storage`, even though only
storage ever raises it: the `RecordRepository` port returns it, and a port in
`pressa-core` cannot name a type from a crate `pressa-core` does not depend on.
That is also why its backend variant carries a `String` — see
[`storage.md`](storage.md) §6. `pressa-storage` names `serde_json` and `chrono`
for the same reason `pressa-app` does: they appear in the port's own signatures.

`pressa-app` carries `serde` and `indexmap` alongside `serde_yaml` because the
config loader deserializes into them directly and builds the `IndexMap` that
`Schema.collections` is declared as; `serde_json` because `Record.data` and the
service signatures are JSON documents; `thiserror` is there because `AppError`
and `ConfigError` are library error types (`AGENTS.md`). None of these is a new
third-party choice — they are the same crates `pressa-core` already uses.

`pressa-app` names `pressa-storage` in `[dev-dependencies]` and nowhere else:
its service tests need an implementation of the `RecordRepository` port, and
`MemoryRepository` is the one that exists for the purpose
([ADR-0009](adr/0009-test-only-dependency-on-pressa-storage.md)). `rusqlite`
stays forbidden to `pressa-app` in every section, dev included — that is the
rule this exception leaves intact, and `tests/architecture.rs` enforces both.

`pressa-tui` names `assert_cmd` and `predicates` in `[dev-dependencies]` and
nowhere else: SPEC-005's criteria are about a real process's exit code and its
stdout/stderr split, which only a spawned binary can show
([ADR-0010](adr/0010-test-only-dependencies-need-an-adr.md)). That ADR also
settles the general question — a test-only crate needs an ADR like any other,
judged on what its tests print when they fail. Unlike ADR-0009 this relaxes no
rule, so `tests/architecture.rs` is unchanged: `pressa-tui`'s only forbidden
dependency is `rusqlite`, in every section.

## 3. Data flow

### 3.1 Startup

```
main.rs                                          ← a shim, nothing else
  → clap parses argv                             (pressa-tui: cli.rs)
  → cli::run
      → resolve the project: --project or walk up  (pressa-app)
      → tracing subscriber → .pressa/pressa.log  (never stdout: the TUI owns it)
      → cli::dev
          → load pressa.yaml        → Schema      (pressa-app)
          → open .pressa/data.db, run migrations  (pressa-storage)
          → build RecordService / CollectionService (pressa-app)
          → enter terminal raw mode + alternate screen
          → run event loop
```

The last two steps are T7's; until then `cli::dev` stops after the services and
returns `CliError::NotImplementedYet` ([SPEC-005](../specs/005-cli.md) screen I).

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
- [ADR-0008](adr/0008-tracing-to-a-log-file.md) — diagnostics to `.pressa/pressa.log`
- [ADR-0009](adr/0009-test-only-dependency-on-pressa-storage.md) — `pressa-app` may reach storage in tests
- [ADR-0010](adr/0010-test-only-dependencies-need-an-adr.md) — test-only dependencies need an ADR too
