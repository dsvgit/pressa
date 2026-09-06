# ADR-0002: Synchronous rusqlite; no async runtime

Status: **Accepted** · Date: 2026-09-06

## Context

Two of the four source plans (`ai/1.md`, `ai/4.md`) proposed `tokio` + `sqlx`,
citing a future HTTP/API layer and Postgres support. The other two
(`ai/2.md`, `ai/3.md`) proposed synchronous `rusqlite`, citing simplicity.

Pressa is a single-user, local, terminal application reading and writing one
SQLite file. There is no network, no concurrent writer, and no request load.

## Decision

Use **synchronous `rusqlite`** with the `bundled` feature. No `tokio`, no
`async` anywhere in the workspace for M0.

## Rationale

- There is no concurrency to manage. An async runtime here buys nothing at
  runtime and costs `async fn` in the `RecordRepository` trait, an executor in
  the event loop, and `#[tokio::test]` in every storage test.
- Synchronous Rust is materially easier to generate correctly. Given that most
  of this code will be written by agents, "fewer ways to be subtly wrong" is a
  real engineering criterion, not a preference.
- `bundled` removes the system SQLite dependency, so a clean machine and CI
  both build with `cargo build` alone.
- Speculative generality is the failure mode this project is most exposed to.
  A runtime added for a REST layer that is five milestones away is a cost paid
  now for a benefit that may never arrive.

## Consequences

- The `RecordRepository` port is a synchronous trait. Introducing async later
  means changing that trait and its two implementations — a contained,
  mechanical change, precisely because the port exists.
- Long operations block the render loop. At MVP data sizes this is
  imperceptible; if it stops being imperceptible, the fix is a worker thread
  and a channel, not a runtime.
- A future REST or MCP server (M5) will need async at its own boundary. It can
  own its runtime and call the synchronous services from a blocking pool.

## Revisit when

Any of: a network-facing server is actually being built; Postgres becomes a
requirement; a single operation blocks the UI for more than ~100 ms. Revisiting
means a new ADR superseding this one, not editing this one.
