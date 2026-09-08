# ADR-0008: Diagnostics go to a log file via `tracing`

Status: **Accepted** · Date: 2026-09-06

## Context

A TUI owns the terminal. From the moment `pressa` enters raw mode and the
alternate screen, anything written to stdout or stderr lands in the middle of
the rendered frame and corrupts it. `AGENTS.md` and
[`architecture.md`](../architecture.md) §3.1 already state the rule — nothing
writes to stdout or stderr while the TUI is running, diagnostics go to
`.pressa/pressa.log` — but the crates that implement it were never recorded as
a decision, and [`development.md`](../development.md) §5 requires an ADR for
every new dependency.

The alternative to a logging framework is not "no logging": it is `println!`
debugging, which is exactly what a TUI cannot afford, or a hand-rolled writer
that reimplements level filtering and structured fields badly.

## Decision

Use **`tracing`** as the instrumentation facade and **`tracing-subscriber`**
(default features off; `fmt` and `std` only) as the sink, writing to
`<project>/.pressa/pressa.log` in append mode. Both belong to `pressa-tui`,
which owns the composition root and initialises the subscriber before anything
else can fail.

`thiserror` is likewise permitted in `pressa-tui`, matching the rule already in
force for the other three crates: `thiserror` in libraries, `anyhow` only in
`main.rs` and tests.

## Rationale

- `tracing` is the facade the rest of the ecosystem instruments against, so
  library-level spans from a future dependency arrive in the same log without
  extra wiring. `log` would work today and be re-plumbed the first time we want
  a span.
- Structured fields (`tracing::info!(version = …, "pressa started")`) survive
  into the log as `version=…` rather than being flattened into prose, which is
  what makes a log file worth grepping after a user reports a bug.
- Default features are off deliberately. `fmt` + `std` excludes `ansi`, so no
  escape sequences reach a file that a human will `cat`, and excludes
  `env-filter`, which drags in `regex`. Level control arrives with `--log-level`
  in T6 and can be spent then.
- Appending rather than truncating means a crash and the run that reproduces it
  are in the same file.
- The subscriber is process-global, so initialising it can only be done once.
  `logging::init` returns a `Result` rather than panicking, and `thiserror`
  gives that error a source chain naming the path that failed — a panic here
  would fire before the terminal guard exists and print a backtrace over
  nothing, but a returned error lets `main` exit cleanly with a message.

## Consequences

- `pressa-tui` depends on `tracing`, `tracing-subscriber` and `thiserror`.
  [`architecture.md`](../architecture.md) §2 records them in the crate table.
- The other three crates get `tracing` only if a task shows they need it; a
  library crate that wants to log should take the facade, never a subscriber.
- Nothing is visible on the terminal by default. A user debugging a startup
  problem is told to read `.pressa/pressa.log`; `pressa init` creates the
  directory, and `logging::init` creates it if it is missing.
- Log rotation does not exist. A long-lived project accumulates one growing
  file.

## Revisit when

Any of: the log file becomes large enough to matter (rotation, or
`tracing-appender`); a non-TUI entry point wants diagnostics on stderr, where
the rule does not apply; `--log-level` needs filtering richer than a level,
which is when `env-filter` earns its `regex`.
