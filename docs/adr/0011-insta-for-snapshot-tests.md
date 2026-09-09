# ADR-0011: `pressa-tui` takes `insta` for snapshot tests

Status: **Accepted** · Date: 2026-09-09

## Context

[ADR-0010](0010-test-only-dependencies-need-an-adr.md) settled the general
rule — a test-only crate needs an ADR like any other, judged on what its tests
print when they fail — and then said, in its consequences, exactly what this
ADR has to answer:

> **`insta` is not pre-approved by this.** T7 wants it — [`tui.md`](../tui.md)
> §8 makes snapshot tests mandatory from the first TUI task — and it is absent
> from `architecture.md` §2 for the same reason `assert_cmd` was. Clause 1
> above says it needs its own decision, taken when T7 starts.

T7 has started. [SPEC-006](../../specs/006-tui-shell.md) specifies eight 80x24
frames and makes nine of its acceptance criteria snapshot assertions, and its
"Preconditions" section blocks implementation on this decision.

The repository has been assuming `insta` in five places for three weeks without
ever deciding on it:

| Where | What it says |
|---|---|
| [`AGENTS.md`](../../AGENTS.md) | "UI work ships with `insta` snapshots" |
| [`tui.md`](../tui.md) §8 | a worked `insta::assert_snapshot!(terminal.backend())` example, "not optional and not deferred" |
| [ADR-0001](0001-ratatui-and-crossterm.md) | ratatui was *chosen* partly because `TestBackend` "combined with `insta`" gives an agent a deterministic signal |
| [`justfile`](../../justfile) | `just snap` is `cargo insta review` |
| [`roadmap.md`](../roadmap.md) §2 | T7's key acceptance is "80x24 snapshot of Home" |

ADR-0010 already ruled that this kind of accumulation is not an approval: it
made the same point about `roadmap.md` naming `assert_cmd` — "that row was
written on 2026-09-06, before any code existed, and a roadmap mention is not an
approval". Five mentions are five mentions, not a decision.

One fact makes the decision less obvious than it looks. `insta` depends on
`tempfile`, and ADR-0010 clause 4 refused `tempfile` by name and made the
refusal general.

## Decision

1. **`pressa-tui` may name `insta` in `[dev-dependencies]`, and nowhere else.**
   The workspace dependency table pins it; no other crate takes it.
2. **Default features only** — `colors` and `console`. Everything else
   (`serde`, `redactions`, `filters`, `glob`, `json`, `yaml`, `ron`, `toml`,
   `csv`, `regex`, `walkdir`) stays off: we snapshot plain strings rendered
   from a `TestBackend`, and a redaction engine over a terminal buffer would be
   a way to hide a failing frame rather than read it.
3. **Snapshots are source.** `.snap` files live in
   `pressa-tui/tests/snapshots/`, are committed, and are reviewed like code. A
   PR that changes a frame must show that diff. `cargo insta review` is the only
   sanctioned way to accept one; a snapshot accepted without a reason in the PR
   body is a review finding, as [`AGENTS.md`](../../AGENTS.md)'s definition of
   done already says.
4. **`tempfile` arriving through `insta` does not reopen clause 4 of
   ADR-0010.** That clause refuses code we would write against `tempfile`, and
   `pressa-app/tests/support/mod.rs`'s hand-rolled `TempTree` stays hand-rolled.
   We decide what our crates *name*, not what our dependencies use internally;
   the alternative is a policy on the transitive graph, which we do not have,
   cannot enforce, and would fail on `rusqlite` first.

## Rationale

- **It passes ADR-0010's test, and by a wider margin than `assert_cmd` did.**
  The failure being diagnosed is one wrong cell in a 1,920-character string. A
  hand-rolled `assert_eq!` prints two walls of box-drawing characters and
  leaves the reader to diff them by eye; `insta` prints a line-by-line diff with
  only the changed rows marked, names the snapshot file, and writes `.snap.new`
  next to it. For a UI that the author cannot see, that diff *is* the review.
- **The review loop is the second half of the value, and it has no substitute.**
  `AGENTS.md` forbids accepting snapshots blindly, and `cargo insta review` is
  what makes that instruction enforceable: accept or reject, one snapshot at a
  time, with the diff on screen. With hand-written expectation files the only
  way to update a frame is to paste the new one over the old one, which is
  precisely the motion that rubber-stamps a regression.
- **The cost is small, off by default, and invisible at run time.** With default
  features `insta` adds seven crates to `Cargo.lock` — `insta`, `console`,
  `similar`, `tempfile`, `fastrand`, `rustix`, `errno`; `libc`, `once_cell`,
  `getrandom`, `cfg-if` and `bitflags` are already there — taking the lock from
  115 packages to 122. None appears in `[dependencies]`, so `cargo build`
  compiles none of them and the shipped binary contains none of them.
- **Colours stay on, unlike `chrono` and `tracing-subscriber`, and for a
  stated reason.** This workspace trims default features by habit. Here the
  coloured diff is the thing being bought, so trimming it would be buying the
  dependency and throwing away the reason for it.
- **CI needs no flag.** `insta` writes `.snap.new` locally and refuses to write
  anything when `CI` is set, so `just ci` fails on a changed frame instead of
  quietly accepting it. The default behaviour is already the policy.

## Alternatives considered

- **Hand-rolled: expected frames as `.txt` files, `assert_eq!`, and our own
  diff.** No dependency, roughly thirty lines. Rejected for the same reason
  ADR-0010 rejected hand-rolling `assert_cmd`, and it is the same shape of
  argument: the thirty lines are the easy part, and the part they reimplement
  badly — a readable diff and a per-snapshot review loop — is the whole point.
  This is not `TempTree`, whose failure mode is a missing directory and needs no
  help to diagnose.
- **`expect-test`.** Smaller tree, inline expectations, `UPDATE_EXPECT=1`.
  Rejected on shape rather than on cost: an 80x24 frame inlined in the test
  source makes the test unreadable, eight of them make the file unreviewable,
  and the update mechanism is an environment variable that rewrites every
  expectation at once — the blind-accept motion we are trying to prevent.
- **No snapshots; unit-test `update` only.** This is the cheapest option and the
  one that removes the project's only pass/fail signal on a screen nobody looks
  at. [`tui.md`](../tui.md) §8 calls it out in advance: "not optional and not
  deferred". Rejected.
- **Refusing `insta` because it pulls `tempfile`.** Rejected as a
  misreading of ADR-0010; see decision 4. Taken seriously enough to write down,
  because the next reader will notice `tempfile` in the lockfile and wonder.

## Consequences

- The workspace dependency table gains `insta = "1"` (resolved: 1.48);
  `pressa-tui/Cargo.toml` gains it under `[dev-dependencies]` beside
  `assert_cmd` and `predicates`. `Cargo.lock` grows by seven packages.
- [`architecture.md`](../architecture.md) §2 gains `insta` in `pressa-tui`'s
  row and a sentence in the note that already covers `assert_cmd`; §7 gains
  this ADR.
- `pressa-tui/tests/architecture.rs` is **unchanged by this decision**, for the
  third time and the same reason: a forbidden edge is what that file checks, and
  `insta` is not one. As with ADR-0010 and unlike ADR-0009, this ADR relaxes no
  existing rule, so it owes no guard test. (T7 does edit that file, for an
  unrelated reason: [SPEC-006](../../specs/006-tui-shell.md) makes `pressa-core`
  a forbidden edge for `pressa-tui`, since the domain types reach it through
  `pressa_app::domain`.)
- `just snap` requires `cargo install cargo-insta` once per machine. That is a
  developer tool, not a workspace dependency: `just ci` passes and fails
  correctly without it, and a machine that lacks it can still read the
  `.snap.new` file the failure leaves behind.
- `cargo test -p pressa-tui` compiles seven more crates. `cargo build` does not.
- `.snap` files become reviewable artifacts of every UI task from T7 onward, and
  the definition of done's "snapshots reviewed, not auto-accepted" acquires a
  tool that makes it checkable.
- [SPEC-006](../../specs/006-tui-shell.md)'s "Preconditions" section is
  satisfied and can be struck when the spec is approved.

## Revisit when

Any of: the test-only roster passes three or four crates, at which point a
standing list beats one ADR each — ADR-0010 named the same threshold and this
ADR takes the roster to three; snapshots outgrow strings, so that a frame needs
image or binary diffing; `ratatui` grows assertion helpers that print a legible
buffer diff of their own, which would make `insta` redundant for frames while
leaving it useful for nothing else here; or a snapshot test goes flaky, which
would point at non-determinism in our own rendering — an `IndexMap` turned
`HashMap`, a timestamp in a frame — and never at `insta`, so the fix belongs in
the renderer and this decision stands.
