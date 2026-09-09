# ADR-0010: Test-only dependencies still need an ADR; `pressa-tui` takes `assert_cmd`

Status: **Accepted** · Date: 2026-09-09

## Context

[`AGENTS.md`](../../AGENTS.md) and [`development.md`](../development.md) §5 say
a new crate in `Cargo.toml` requires an ADR, because dependencies are
architecture. Neither exempts `[dev-dependencies]`.

T6 ([SPEC-005](../../specs/005-cli.md)) is the first task that wants one.
Roughly fifteen of its acceptance criteria have the same shape — this command
exits with this code, this text went to *that* stream, the other stream was
empty — and the only way to observe an exit code and a stream split is to spawn
the real binary. [`roadmap.md`](../roadmap.md) §2 names `assert_cmd` as T6's key
acceptance, but that row was written on 2026-09-06, before any code existed, and
a roadmap mention is not an approval.

The precedent in the repository points the other way, and it was set in code
during T5. [`pressa-app/tests/support/mod.rs`](../../pressa-app/tests/support/mod.rs)
hand-rolls a temporary-directory helper with the comment: "Hand-rolled rather
than `tempfile`: a new dependency needs an ADR (`AGENTS.md`), and this is twenty
lines."

[ADR-0009](0009-test-only-dependency-on-pressa-storage.md) has already ruled on
the general point once: dev sections are real. The guard in
[`pressa-tui/tests/architecture.rs`](../../pressa-tui/tests/architecture.rs)
scans `[dev-dependencies]` deliberately, and allowing `pressa-app` a single
test-only edge to `pressa-storage` cost a written exception rather than a shrug.

So T6 cannot start without deciding two things that have been left implicit: is
a test-only crate subject to the rule at all, and if so, does `assert_cmd`
clear it.

## Decision

1. **A test-only dependency needs an ADR, like any other.** Being confined to
   `[dev-dependencies]` is not an exemption, and a mention in `roadmap.md` or a
   spec is not an approval.
2. **It is judged against a lighter test than a runtime dependency.** A crate
   admitted this way must be absent from `[dependencies]` and
   `[build-dependencies]`, must not be reachable from any `src/` file, and must
   not appear in the shipped binary.
3. **`pressa-tui` may name `assert_cmd` and `predicates` in
   `[dev-dependencies]`, and nowhere else.**
4. **The `tempfile` refusal stands and generalises.** A crate whose whole job is
   code we would otherwise own in twenty readable lines is still refused, in
   every section. Why that does not also refuse `assert_cmd` is the next
   section.

This ADR decides the rule and admits two crates under it. It is not a roster:
the next test-only crate is a new decision.

## Rationale

- What separates `assert_cmd` from `tempfile` is not size but **the failure mode
  of the test it replaces**. A hand-rolled `assert_eq!(out.status.code(),
  Some(1))` fails with `left: Some(0), right: Some(1)` and throws away the
  captured stderr — the one thing that would say why the CLI did something else.
  `assert_cmd` prints the command, the code and both streams on failure.
  SPEC-005 makes that assertion about fifteen times; a harness that reports each
  of those failures uninformatively produces CLI tests that get deleted the
  first time one goes red for an unclear reason.
- `TempTree`'s failure mode, by contrast, is a missing directory, which needs no
  help to diagnose. Twenty lines were the right call there and are the wrong
  call here. The distinguishing question for future proposals is not "how much
  code does it save" but "what does the test print when it fails".
- Resolving the binary is *not* the hard part, and the ADR should not pretend it
  is: `env!("CARGO_BIN_EXE_pressa")` works from an integration test of the crate
  that defines the bin. The assertion is the hard part.
- `predicates` is not incidental baggage. `assert_cmd`'s stream assertions take
  a predicate, and `predicate::str::contains` is how a criterion like "stderr
  contains `collections.posts.fields[3].type`" is written. Admitting
  `assert_cmd` without it means asserting on raw bytes, which is the thing being
  avoided.
- Both are invisible to users: `cargo build` does not compile a dev-dependency,
  so neither reaches the binary, its size, or its supply chain at run time.

## Alternatives considered

- **`std::process::Command` with `env!("CARGO_BIN_EXE_pressa")`.** No
  dependency, no ADR, about six more lines per test. Rejected for the
  diagnostics above, not for the line count. Choosing it would also mean
  correcting `roadmap.md` §2, which names `assert_cmd`.
- **A blanket exemption for `[dev-dependencies]`.** Rejected. It would delete
  the rule ADR-0009 had just spent an ADR narrowing, and test dependencies are
  still compile time, still supply chain, and still a place an agent can put
  work that nobody reviews.
- **Testing the CLI only in-process, through `cli::run(cli, cwd)`.** Cheapest of
  all: no dependency and no spawn. It cannot observe a real process's exit code
  or the stdout/stderr split, which is exactly what SPEC-005's cross-cutting
  criteria are about. Kept as the majority of T6's tests, not as all of them.

## Consequences

- `pressa-tui/Cargo.toml` gains `assert_cmd` and `predicates` under
  `[dev-dependencies]`; the workspace dependency table pins both.
- [`architecture.md`](../architecture.md) §2 gains a note for `pressa-tui`, the
  way it already carries one for `pressa-app`'s test-only edge.
- `pressa-tui/tests/architecture.rs` is **unchanged**. `pressa-tui`'s only
  forbidden dependency is `rusqlite`, checked in every section; neither new
  crate is a forbidden edge, so no guard test is owed. This is the difference
  from ADR-0009, which had to relax an existing rule.
- `cargo test -p pressa-tui` compiles two more crates. `cargo build` does not.
- SPEC-005's "Preconditions" section is satisfied and can be struck.
- **`insta` is not pre-approved by this.** T7 wants it —
  [`tui.md`](../tui.md) §8 makes snapshot tests mandatory from the first TUI
  task — and it is absent from `architecture.md` §2 for the same reason
  `assert_cmd` was. Clause 1 above says it needs its own decision, taken when T7
  starts.

## Revisit when

Any of: a test-only crate is proposed that has a runtime footprint after all (a
proc macro that lands in the binary, or a build script); the roster grows past
three or four crates, at which point a standing list beats one ADR each; or the
CLI's surface grows enough that snapshotting `--help` as a file — `trycmd` and
its kind — beats asserting on strings, which would supersede the `assert_cmd`
half of this decision.
