# AGENTS.md

Rules for anyone — human or agent — writing code in this repository. Read this
before touching anything. The reasoning behind these rules is in
[`docs/development.md`](docs/development.md); this file is the enforceable
short form.

## Project

Pressa is a terminal-native, schema-driven data manager written in Rust.
Collections are declared in `pressa.yaml`; the TUI is generated from that
schema. Current milestone: **M0**, see [`docs/roadmap.md`](docs/roadmap.md).

## Before you write code

1. Read the spec for your task in `specs/`. If it is not `Approved`, stop and
   say so.
2. Read the ADRs it references in `docs/adr/`.
3. Read the existing code and tests you are about to change.
4. Write the failing tests from the acceptance criteria **first**.

## Architecture rules

```
pressa-tui → pressa-app → pressa-core ← pressa-storage
```

- `pressa-core` must not depend on ratatui, crossterm, rusqlite, clap or tokio.
- `pressa-tui` must not depend on rusqlite except in `main.rs` (composition root).
- `pressa-storage` must not depend on ratatui or clap.
- All user actions are `Command` variants; key handling lives only in the
  keymap table.
- `update(&mut AppState, Command) -> Vec<Effect>` is pure. No I/O inside it.
- `view(&AppState, frame)` is pure. It reads state and nothing else.
- No code may branch on a specific collection slug. Ever.
  ([ADR-0004](docs/adr/0004-schema-driven-ui.md))

## Code rules

- No `unwrap()`, `expect()` or `panic!()` in library code outside `#[cfg(test)]`.
- `thiserror` in libraries; `anyhow` only in `main.rs` and tests.
- No `async`, no `tokio`. ([ADR-0002](docs/adr/0002-synchronous-rusqlite.md))
- Nothing writes to stdout or stderr while the TUI is running. `tracing` goes
  to `.pressa/pressa.log`.
- Prefer small modules and explicit types over clever generics.
- New behaviour ships with tests. UI work ships with `insta` snapshots.
- Comment for a reader who is new to Rust: a short single-line `//` comment on
  every non-obvious line — borrows, lifetimes, `?`, iterators, closures, trait
  bounds, `match` arms that are not self-evident. English, one line, no comment
  blocks. Say what the line does and why, never restate the syntax
  (`// take a shared borrow so the row is not moved`, not `// call iter()`).

## The stop rule

> If you discover while implementing that the architecture is wrong:
> **stop, do not redesign.** Write down the problem, propose an ADR, and ask.

Never redesign inside a feature diff. Never add a dependency without an ADR.

## Scope

M0 field types — and no others: `text`, `textarea`, `number`, `boolean`,
`datetime`, `select`, `json`.

Not in M0. Adding any of these is a scope violation, not an improvement:

auth · RBAC · multi-user · REST · GraphQL · MCP · web UI · plugins · hooks ·
scripting · WASM · rich text · media · uploads · drafts · versioning ·
localization · relation fields · filters · sort UI · bulk operations ·
command palette · Postgres · MongoDB · remote sync · realtime · FTS5 · async

The full list with reasons: [`docs/roadmap.md`](docs/roadmap.md) §3.

## Before you finish

```
just ci      # fmt --check + clippy -D warnings + test --workspace
```

Then check the diff against the spec's acceptance criteria, one by one, and set
the spec's status to `Implemented`. If you built something the spec did not ask
for, remove it.

If your change alters what the CLI does or prints, update the `sandbox` recipe
in the [`justfile`](justfile) in the same PR and run `just sandbox` — it is the
directory a human drives the binary in, and its `README.md` promises what each
command prints. Fix it there, never in `/tmp`
([`docs/development.md`](docs/development.md) §8).

## Definition of done

- [ ] Every acceptance criterion has a test that fails without your change
- [ ] `just ci` is green
- [ ] No `unwrap()` outside tests, no new dependency without an ADR
- [ ] Crate boundaries respected
- [ ] `just sandbox` still shows what changed, if the CLI changed
- [ ] Snapshots reviewed, not auto-accepted
- [ ] Spec status updated
- [ ] Nothing extra implemented
