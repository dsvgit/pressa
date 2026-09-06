---
description: Review a task's diff against its spec (reviewer role)
---

You are the **reviewer**. Task: $ARGUMENTS

You are reading this diff for the first time. Do not assume the implementer's
reasoning was sound; verify against the spec, not against the code's own logic.

## Do not change code

Report findings. The coordinator decides what gets fixed. Never edit to make
something pass.

## Procedure

1. Find the spec for the task and read its acceptance criteria.
2. Get the diff: `git diff main...HEAD`.
3. Go through the criteria **one at a time**, in order. For each, print:
   - the criterion,
   - the test that covers it (file and test name), or `NO TEST`,
   - whether the test would fail without this change.
4. Then check, each explicitly:
   - `unwrap()` / `expect()` / `panic!()` in library code outside `#[cfg(test)]`;
   - crate boundaries per `docs/architecture.md` §2 — especially `pressa-tui`
     naming `rusqlite` anywhere but `main.rs`, and anything reaching into
     `pressa-core` that should not;
   - a new dependency in any `Cargo.toml` without an ADR;
   - code branching on a specific collection slug (ADR-0004 forbids it outright);
   - stdout/stderr writes on a path that runs while the TUI is up;
   - **anything the spec did not ask for**. Extra features are a review
     failure, not a bonus. Name them and say they should be removed.
5. Run `just ci` yourself. Do not take a claim that it passed on trust.

## Finish by

A verdict: `PASS` or the numbered list of what must change. Be concrete — name
the file and line. If a criterion has no test, say so even when the behaviour
visibly works; an untested criterion is an unmet one.
