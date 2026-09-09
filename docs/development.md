# Development Process

Status: Approved · Last updated: 2026-09-06

This document describes how work happens. [`AGENTS.md`](../AGENTS.md) is the
short, enforceable version that agents read on every task; this is the
reasoning behind it.

## 1. Spec-driven development

```
spec  →  acceptance criteria  →  failing tests  →  implementation  →  review
```

The reason is specific to working with AI agents: an agent given "build the
record editor" will produce something plausible and unreviewable. An agent
given twelve acceptance criteria produces something you can check line by line.
The spec is not documentation of the code — it is the definition of done,
written before the code exists.

Rules:

- No implementation starts before its spec is **Approved**.
- The code must satisfy the acceptance criteria **and nothing more**. Extra
  features are a review failure, not a bonus.
- Behaviour change without a spec update is incomplete work.

Spec lifecycle: `Draft → Approved → In Progress → Implemented`. The status
lives in the spec's header.

## 2. Specs vs ADRs

Two kinds of documents, different lifetimes, do not mix them:

| | `specs/` | `docs/adr/` |
|---|---|---|
| Answers | what this feature does | why the project is built this way |
| Scope | one feature | one architectural decision |
| Changes | often, per feature | rarely; superseded, never edited |
| Example | "record editor saves on Ctrl+S" | "we use synchronous rusqlite" |

An ADR is immutable once accepted. Changing your mind means writing a new ADR
that supersedes it, so the history of reasoning survives.

## 3. Specs are written just in time

`specs/001`–`004` (config, validation, storage, services) are written up front
— that layer is stable and everything depends on it. The TUI specs
(`005`–`008`) are written immediately before their task, because a UI spec
written four tasks early describes a screen whose state model has since
changed. A stale spec is worse than a missing one: it gets implemented.

## 4. The task cycle

```
 1. read the spec
 2. read the ADRs it references
 3. read the existing code and tests it touches
 4. write the failing tests from the acceptance criteria
 5. implement until green
 6. just ci
 7. review the diff against the acceptance criteria, one by one
 8. update the spec status → Implemented
 9. commit on a branch, PR, merge
```

Steps 4 and 7 are the ones agents skip and the ones that matter. Tests written
after the implementation test what the code does, not what the spec asked for.

## 5. The stop rule

> If, while implementing a task, you discover that the architecture is wrong:
> **stop. Do not redesign.** Write the problem down, propose an ADR, and ask.

This is the single most important rule for agent-driven work. An agent that
redesigns while implementing produces a diff where the feature and the
architecture change together and neither can be reviewed. Architectural drift
across ten tasks is how projects like this die.

The same rule applies to dependencies: a new crate in `Cargo.toml` requires an
ADR, because dependencies are architecture.

## 6. Agent roles

Five roles, one human coordinator. Roles are *hats*, not autonomous processes —
they do not talk to each other, they report to the coordinator.

| Role | Does | Never does |
|---|---|---|
| **spec-writer** | Turns a request into a spec file using the template. Asks about ambiguity instead of guessing. | Writes code |
| **architect** | Writes ADRs, decomposes a spec into tasks, checks that a proposed change respects crate boundaries. | Writes production code |
| **implementer** | Implements exactly one task: tests first, then code, then `just ci`. | Redesigns; touches crates the task does not name; adds dependencies |
| **tester** | Turns acceptance criteria into unit, integration and snapshot tests. Hunts edge cases the spec left implicit. | Changes production code to make tests pass |
| **reviewer** | Checks the diff against the spec's acceptance criteria, the ADRs and the boundary rules. Flags unnecessary abstraction. | Approves work it did not verify by running |

What we explicitly do **not** do: five autonomous agents messaging each other.
That produces plausible-looking consensus and no accountability. One
coordinator, one task at a time, human review at the merge point.

## 7. Branches and commits

One task, one branch, one PR — even working solo. The PR body links the spec
and lists the acceptance criteria with checkboxes.

```
main
 ├── feat/t02-schema-config
 ├── feat/t04-sqlite-repository
 └── feat/t09-record-editor
```

Solo PRs are not bureaucracy here: they are the unit at which an agent's work
is reviewable in one sitting. A task too large for one PR was decomposed badly.

## 8. Commands

```
just fmt      cargo fmt --all
just lint     cargo clippy --workspace --all-targets -- -D warnings
just test     cargo test --workspace
just ci       fmt --check + lint + test
just snap     cargo insta review
just run      cargo run -p pressa-tui -- dev --project examples/blog
just sandbox  rebuild /tmp/pressa-sandbox: throwaway projects to drive by hand
just hooks    git config core.hooksPath .githooks
```

`just sandbox` is how a change is looked at rather than asserted about. It
generates `/tmp/pressa-sandbox` — a `pressa` wrapper that rebuilds before every
run, an empty directory for `init`, a copy of `examples/blog` for `validate` and
`dev`, a deliberately broken schema, and a nested tree for discovery — with a
`README.md` listing the commands worth trying and what each should print. It
lives in `/tmp` because everything in it is disposable; running the recipe again
wipes it, experiments included.

Generated, not committed, and for a reason: a checked-in sandbox is a second
copy of `examples/blog` and of the CLI's behaviour, and both drift. Two rules
keep it honest, and they are part of the definition of done below:

- **The recipe is the sandbox.** Fix it in the `justfile`, never in `/tmp`.
- **A task that changes what the CLI prints updates the recipe in the same
  PR** — the flags it demonstrates, the transcripts its README promises, the
  fixtures it writes. The reviewer runs `just sandbox` and follows its README;
  a promise that no longer holds is a review finding like any other. T7 replacing
  `dev`'s `the TUI arrives in T7` stub is the first such change due.

Run `just hooks` once per clone. It points git at [`.githooks/`](../.githooks),
whose `pre-commit` runs `rustfmt` over the staged `.rs` files and re-stages
them, so a commit is formatted by construction rather than by remembering. A
file that is only partially staged is not touched — the hook reports it and
aborts instead of quietly staging the rest of your working copy. `git commit
--no-verify` skips the hook. It is not a substitute for `just ci`: it does not
lint or test.

`just ci` must pass before any task is considered finished. CI runs the same
three commands, so a green local run means a green pipeline.

## 9. Definition of done

A task is done when all of these hold:

- [ ] every acceptance criterion in the spec has a test that fails without the change;
- [ ] `just ci` is green;
- [ ] no `unwrap()` / `expect()` / `panic!()` in library code outside tests;
- [ ] no new dependency without an ADR;
- [ ] crate boundaries respected (see [`architecture.md`](architecture.md) §2);
- [ ] snapshots reviewed deliberately, not auto-accepted;
- [ ] `just sandbox` still demonstrates what the change did to the CLI (§8);
- [ ] the spec's status is `Implemented`;
- [ ] nothing implemented that the spec did not ask for.
